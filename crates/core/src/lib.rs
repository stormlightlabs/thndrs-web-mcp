//! Core types and shared functionality for mcp-web.
//!
//! This crate provides:
//! - Cache implementation with SQLite backend
//! - Unified error types
//! - Configuration structures

pub mod cache;
pub mod config;
pub mod error;

pub use cache::{CacheDb, Snapshot};
pub use config::{AppConfig, ConfigError};
pub use error::Error;
