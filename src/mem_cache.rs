// SPDX-License-Identifier: MIT OR Apache-2.0
// This file is part of Static Web Server.
// See https://static-web-server.net/ for more information
// Copyright (C) 2019-present Jose Quintana <joseluisq.net>

//! It provides in-memory files cache functionality.
//!

use bytes::BytesMut;
use compact_str::CompactString;
use headers::{
    AcceptRanges, ContentLength, ContentRange, ContentType, HeaderMap, HeaderMapExt, LastModified,
};
use hyper::{Body, Response, StatusCode};
use once_cell::sync::Lazy;
use sieve_cache::SieveCache;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;

use crate::conditional_headers::{ConditionalBody, ConditionalHeaders};
use crate::file_response::{bytes_range, BadRangeError};
use crate::file_stream::FileStream;

// In-memory files cache capacity.
const CACHE_CAPACITY: usize = 512;

/// Maximum file size to be cached in memory (default `8MB`).
pub(crate) const CACHE_MAX_FILE_SIZE: u64 = 1024 * 1024 * 8;

/// The in-memory files cache that holds all files and provides cache eviction policy.
pub(crate) static CACHE_STORE: Lazy<Mutex<SieveCache<CompactString, MemFile>>> =
    Lazy::new(|| Mutex::new(SieveCache::new(CACHE_CAPACITY).unwrap()));

/// In-memory file representation which will be store in the cache.
pub(crate) struct MemFile {
    /// Buffer size of current file.
    pub buf_size: usize,
    /// Bytes of the current file.
    pub data: BytesMut,
    /// `Content-Type` header of current file.
    pub content_type: ContentType,
    /// `Last Modified` header of current file.
    pub last_modified: Option<LastModified>,
}

pub(crate) fn mem_cache_response_body(
    file: &MemFile,
    headers: &HeaderMap,
) -> Result<Response<Body>, StatusCode> {
    let conditionals = ConditionalHeaders::new(headers);
    let modified = file.last_modified;

    match conditionals.check(modified) {
        ConditionalBody::NoBody(resp) => Ok(resp),
        ConditionalBody::WithBody(range) => {
            let buf = file.data.clone().freeze();
            let mut len = buf.len() as u64;
            let mut reader = std::io::Cursor::new(buf);
            let buf_size = file.buf_size;

            bytes_range(range, len)
                .map(|(start, end)| {
                    match reader.seek(SeekFrom::Start(start)) {
                        Ok(_) => (),
                        Err(err) => {
                            tracing::error!("seek file from start error: {:?}", err);
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }
                    };

                    let sub_len = end - start;
                    let reader = reader.take(sub_len);
                    let stream = FileStream {
                        reader,
                        buf_size,
                        file_path: None,
                    };
                    let body = Body::wrap_stream(stream);
                    let mut resp = Response::new(body);

                    if sub_len != len {
                        *resp.status_mut() = StatusCode::PARTIAL_CONTENT;
                        resp.headers_mut().typed_insert(
                            match ContentRange::bytes(start..end, len) {
                                Ok(range) => range,
                                Err(err) => {
                                    tracing::error!("invalid content range error: {:?}", err);
                                    let mut resp = Response::new(Body::empty());
                                    *resp.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
                                    resp.headers_mut()
                                        .typed_insert(ContentRange::unsatisfied_bytes(len));
                                    return Ok(resp);
                                }
                            },
                        );

                        len = sub_len;
                    }

                    resp.headers_mut().typed_insert(ContentLength(len));
                    resp.headers_mut().typed_insert(file.content_type.clone());
                    resp.headers_mut().typed_insert(AcceptRanges::bytes());

                    if let Some(last_modified) = modified {
                        resp.headers_mut().typed_insert(last_modified);
                    }

                    Ok(resp)
                })
                .unwrap_or_else(|BadRangeError| {
                    // bad byte range
                    let mut resp = Response::new(Body::empty());
                    *resp.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
                    resp.headers_mut()
                        .typed_insert(ContentRange::unsatisfied_bytes(len));
                    Ok(resp)
                })
        }
    }
}
