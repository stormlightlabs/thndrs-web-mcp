//! Client code for mcp-web.
//!
//! This crate provides HTTP fetch pipeline, content extraction, and related
//! functionality shared by the server and CLI.

pub mod extract;
pub mod fetch;

pub use extract::{
    ExtractConfig, ExtractedDoc, ExtractionResult, Extractor, LectitoExtractor, Link, extract_links, extract_readable,
    normalize_markdown,
};

pub use fetch::{FetchClient, FetchConfig, FetchResponse};
