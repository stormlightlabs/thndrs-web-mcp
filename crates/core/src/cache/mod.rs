//! SQLite-backed cache for document snapshots and search results.
//!
//! This module provides a persistent, content-addressed cache using SQLite
//! with async access via tokio-rusqlite. It supports:
//!
//! - Content-addressed storage using SHA-256 hashing
//! - Automatic schema migrations
//! - WAL mode for concurrent access
//! - Multiple purge strategies (age, domain, LRU)

pub mod connection;
pub mod hash;
pub mod migrations;
pub mod search;
pub mod snapshots;

pub use crate::Error;

pub use connection::CacheDb;
pub use search::SearchCacheMeta;
pub use snapshots::Snapshot;
