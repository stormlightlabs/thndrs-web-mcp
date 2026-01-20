//! Brave Search API request types and validation.

use serde::{Deserialize, Serialize};

/// Search request parameters for Brave Web Search API.
///
/// Based on Brave Web Search API documentation:
/// https://api-dashboard.search.brave.com/app/documentation/web-search/get-started
#[derive(Debug, Clone, Serialize, Default)]
pub struct SearchRequest {
    /// Search query (required, max 400 chars / 50 words).
    pub q: String,

    /// Number of results (1-20, default 20).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u8>,

    /// Page offset (0-9, default 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u8>,

    /// Freshness filter: pd|pw|pm|py or YYYY-MM-DDtoYYYY-MM-DD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,

    /// Safe search: off|moderate|strict (default moderate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safesearch: Option<SafeSearch>,

    /// Country code (ISO 3166-1 alpha-2, e.g., "US").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,

    /// Content language (ISO 639-1, e.g., "en").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_lang: Option<String>,

    /// UI/response metadata language (e.g., "en-US").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_lang: Option<String>,

    /// Enable up to 5 extra snippets per result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_snippets: Option<bool>,

    /// Goggles URL or inline definition for custom re-ranking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goggles: Option<String>,

    /// Enable spell-check on query.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spellcheck: Option<bool>,
}

/// Safe search filtering levels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SafeSearch {
    Off,
    Moderate,
    Strict,
}

impl SearchRequest {
    /// Validate the search request parameters.
    ///
    /// Returns an error if any parameters are out of range or malformed.
    pub fn validate(&self) -> Result<(), crate::brave::BraveError> {
        use crate::brave::BraveError;

        if self.q.is_empty() {
            return Err(BraveError::InvalidQuery("query cannot be empty".to_string()));
        }

        if self.q.len() > 400 {
            return Err(BraveError::InvalidQuery(format!(
                "query too long: {} chars (max 400)",
                self.q.len()
            )));
        }

        let word_count = self.q.split_whitespace().count();
        if word_count > 50 {
            return Err(BraveError::InvalidQuery(format!(
                "query too long: {} words (max 50)",
                word_count
            )));
        }

        if let Some(count) = self.count
            && !(1..=20).contains(&count)
        {
            return Err(BraveError::InvalidCount);
        }

        if let Some(offset) = self.offset
            && offset > 9
        {
            return Err(BraveError::InvalidOffset);
        }

        if let Some(freshness) = &self.freshness {
            Self::validate_freshness(freshness)?;
        }

        Ok(())
    }

    /// Validate freshness parameter format.
    fn validate_freshness(freshness: &str) -> Result<(), crate::brave::BraveError> {
        const VALID_PRESETS: &[&str] = &["pd", "pw", "pm", "py"];

        if VALID_PRESETS.contains(&freshness) {
            return Ok(());
        }

        if freshness.len() == 22 && freshness.contains("to") {
            let parts: Vec<&str> = freshness.split("to").collect();
            if parts.len() == 2 {
                let date_regex = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
                if date_regex.is_match(parts[0]) && date_regex.is_match(parts[1]) {
                    return Ok(());
                }
            }
        }

        Err(crate::brave::BraveError::InvalidFreshness(freshness.to_string()))
    }

    /// Get the effective count (default 20).
    pub fn get_count(&self) -> u8 {
        self.count.unwrap_or(20)
    }

    /// Get the effective offset (default 0).
    pub fn get_offset(&self) -> u8 {
        self.offset.unwrap_or(0)
    }

    /// Get the effective safesearch setting (default Moderate).
    pub fn get_safesearch(&self) -> SafeSearch {
        self.safesearch.unwrap_or(SafeSearch::Moderate)
    }
}

#[cfg(test)]
mod tests {
    use crate::BraveError;

    use super::*;

    #[test]
    fn test_valid_request() {
        let req = SearchRequest { q: "test query".to_string(), count: Some(10), offset: Some(0), ..Default::default() };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_empty_query() {
        let req = SearchRequest { q: "".to_string(), ..Default::default() };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_query_too_long_chars() {
        let req = SearchRequest { q: "a".repeat(401), ..Default::default() };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_invalid_count() {
        let req = SearchRequest { q: "test".to_string(), count: Some(25), ..Default::default() };
        assert!(matches!(req.validate(), Err(BraveError::InvalidCount)));
    }

    #[test]
    fn test_invalid_offset() {
        let req = SearchRequest { q: "test".to_string(), offset: Some(10), ..Default::default() };
        assert!(matches!(req.validate(), Err(BraveError::InvalidOffset)));
    }

    #[test]
    fn test_valid_freshness_presets() {
        for freshness in &["pd", "pw", "pm", "py"] {
            let req = SearchRequest {
                q: "test".to_string(),
                freshness: Some((*freshness).to_string()),
                ..Default::default()
            };
            assert!(req.validate().is_ok(), "freshness {} should be valid", freshness);
        }
    }

    #[test]
    fn test_valid_freshness_custom() {
        let req = SearchRequest {
            q: "test".to_string(),
            freshness: Some("2024-01-01to2024-12-31".to_string()),
            ..Default::default()
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_invalid_freshness() {
        let req = SearchRequest { q: "test".to_string(), freshness: Some("invalid".to_string()), ..Default::default() };
        assert!(matches!(req.validate(), Err(BraveError::InvalidFreshness(_))));
    }

    #[test]
    fn test_defaults() {
        let req = SearchRequest { q: "test".to_string(), ..Default::default() };
        assert_eq!(req.get_count(), 20);
        assert_eq!(req.get_offset(), 0);
        assert_eq!(req.get_safesearch(), SafeSearch::Moderate);
    }
}
