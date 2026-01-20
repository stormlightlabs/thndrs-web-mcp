//! Content-addressed cache key generation.

use sha2::{Digest, Sha256};

/// Compute a content-addressed cache key for a document snapshot.
pub fn compute_cache_key(url: &str, vary_headers: &str, mode: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    hasher.update(b"\n");
    hasher.update(vary_headers.as_bytes());
    hasher.update(b"\n");
    hasher.update(mode.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_stability() {
        let hash1 = compute_cache_key("https://example.com", "", "readable");
        let hash2 = compute_cache_key("https://example.com", "", "readable");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_different_mode() {
        let hash_raw = compute_cache_key("https://example.com", "", "raw");
        let hash_readable = compute_cache_key("https://example.com", "", "readable");
        assert_ne!(hash_raw, hash_readable);
    }

    #[test]
    fn test_hash_different_headers() {
        let hash1 = compute_cache_key("https://example.com", "gzip", "readable");
        let hash2 = compute_cache_key("https://example.com", "br", "readable");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_format() {
        let hash = compute_cache_key("https://example.com", "", "readable");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
