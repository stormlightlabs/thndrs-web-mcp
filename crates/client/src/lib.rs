//! Client code for mcp-web.
//!
//! This crate provides HTTP fetch pipeline, content extraction, and related
//! functionality shared by the server and CLI.

pub mod brave;
pub mod extract;
pub mod fetch;

#[cfg(feature = "render")]
pub mod render;

pub use brave::{
    BraveClient, BraveConfig, BraveError, QueryMeta, SafeSearch, SearchRequest, SearchResponse, SearchResult,
};
pub use extract::{
    ExtractConfig, ExtractedDoc, ExtractionResult, Extractor, LectitoExtractor, Link, extract_links, extract_readable,
    normalize_markdown,
};

pub use fetch::{FetchClient, FetchConfig, FetchResponse};

#[cfg(feature = "render")]
pub use render::{HeadlessRenderer, RenderError, RenderOptions, RenderedPage, Renderer};
