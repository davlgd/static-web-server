// SPDX-License-Identifier: MIT OR Apache-2.0
// This file is part of Static Web Server.
// See https://static-web-server.net/ for more information
// Copyright (C) 2019-present Jose Quintana <joseluisq.net>

//! It provides in-memory files cache functionality.
//!

use bytes::BytesMut;
use compact_str::CompactString;
use headers::{ContentType, LastModified};
use once_cell::sync::Lazy;
use sieve_cache::SieveCache;
use std::sync::Mutex;

// In-memory files cache capacity.
const MAX_CACHE_SIZE: usize = 100000;

/// Maximum file size to be cached in memory (default 64MB).
const _MAX_CACHE_FILE_SIZE: u64 = 67_108_864;

/// The in-memory files cache that holds all files and provides cache eviction policy.
pub(crate) static MEM_CACHE: Lazy<Mutex<SieveCache<CompactString, MemFile>>> =
    Lazy::new(|| Mutex::new(SieveCache::new(MAX_CACHE_SIZE).unwrap()));

#[derive(Debug)]
/// In-memory file representation which will be store in the cache.
pub(crate) struct MemFile {
    /// Buffer size of current file.
    pub buf_size: usize,
    /// Bytes of the current file.
    pub bytes: BytesMut,
    /// `Content-Type` header of current file.
    pub content_type: ContentType,
    /// `Last Modified` header of current file.
    pub last_modified: Option<LastModified>,
}
