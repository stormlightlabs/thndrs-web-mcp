//! URL canonicalization for consistent caching and safety checks.

/// Error type for URL canonicalization failures.
#[derive(Debug, Clone, thiserror::Error)]
pub enum UrlError {
    #[error("empty URL")]
    Empty,

    #[error("unsupported scheme: {0}")]
    UnsupportedScheme(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),
}

/// Canonicalize a URL string for consistent caching and safety checks.
///
/// Normalization steps:
/// 1. Trim leading/trailing whitespace
/// 2. Default scheme to https:// if missing
/// 3. Lowercase the host
/// 4. Remove fragment (#...)
/// 5. Keep query string intact (do not reorder)
pub fn canonicalize(input: &str) -> Result<url::Url, UrlError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(UrlError::Empty);
    }

    let url_str = if trimmed.contains("://") { trimmed.to_string() } else { format!("https://{trimmed}") };

    let mut parsed = url::Url::parse(&url_str).map_err(|e| UrlError::InvalidUrl(e.to_string()))?;

    match parsed.scheme() {
        "http" | "https" => {}
        scheme => return Err(UrlError::UnsupportedScheme(scheme.to_string())),
    }

    if let Some(mut host) = parsed.host_str() {
        let h = host.to_lowercase();
        host = h.as_str();
        parsed
            .set_host(Some(host))
            .map_err(|e| UrlError::InvalidUrl(e.to_string()))?;
    }

    parsed.set_fragment(None);

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_basic() {
        let url = canonicalize("https://example.com").unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("example.com"));
    }

    #[test]
    fn test_canonicalize_default_scheme() {
        let url = canonicalize("example.com").unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("example.com"));
    }

    #[test]
    fn test_canonicalize_lowercase_host() {
        let url = canonicalize("https://EXAMPLE.COM").unwrap();
        assert_eq!(url.host_str(), Some("example.com"));
    }

    #[test]
    fn test_canonicalize_remove_fragment() {
        let url = canonicalize("https://example.com#section").unwrap();
        assert_eq!(url.fragment(), None);
        assert_eq!(url.path(), "/");
    }

    #[test]
    fn test_canonicalize_preserve_query() {
        let url = canonicalize("https://example.com?a=1&b=2").unwrap();
        assert_eq!(url.query(), Some("a=1&b=2"));
    }

    #[test]
    fn test_canonicalize_trim_whitespace() {
        let url = canonicalize("  https://example.com  ").unwrap();
        assert_eq!(url.as_str(), "https://example.com/");
    }

    #[test]
    fn test_canonicalize_unsupported_scheme() {
        let result = canonicalize("file:///etc/passwd");
        assert!(matches!(result, Err(UrlError::UnsupportedScheme(_))));
    }

    #[test]
    fn test_canonicalize_empty() {
        let result = canonicalize("");
        assert!(matches!(result, Err(UrlError::Empty)));
    }

    #[test]
    fn test_canonicalize_whitespace_only() {
        let result = canonicalize("   ");
        assert!(matches!(result, Err(UrlError::Empty)));
    }

    #[test]
    fn test_canonicalize_http_allowed() {
        let url = canonicalize("http://example.com").unwrap();
        assert_eq!(url.scheme(), "http");
    }

    #[test]
    fn test_canonicalize_complex_path() {
        let url = canonicalize("https://example.com/path/to/resource?query=value#fragment").unwrap();
        assert_eq!(url.path(), "/path/to/resource");
        assert_eq!(url.query(), Some("query=value"));
        assert_eq!(url.fragment(), None);
    }
}
