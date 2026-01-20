//! Configuration validation rules.
//!
//! This module provides validation logic for `AppConfig` values
//! after they have been loaded from environment, files, or defaults.

use crate::config::AppConfig;
use thiserror::Error;

/// Configuration validation errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to load configuration: {0}")]
    LoadFailed(String),

    #[error("invalid configuration: {field} - {reason}")]
    Invalid { field: String, reason: String },

    #[error("missing required configuration: {field} ({hint})")]
    Missing { field: String, hint: String },
}

impl AppConfig {
    /// Validate configuration values after loading.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::Invalid` if:
    /// - `max_bytes` is 0 or exceeds 50MB
    /// - `timeout_ms` is less than 100ms or exceeds 5 minutes
    /// - `user_agent` is empty
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_bytes == 0 {
            return Err(ConfigError::Invalid { field: "max_bytes".into(), reason: "must be greater than 0".into() });
        }
        if self.max_bytes > 50 * 1024 * 1024 {
            return Err(ConfigError::Invalid { field: "max_bytes".into(), reason: "must not exceed 50MB".into() });
        }

        if self.timeout_ms < 100 {
            return Err(ConfigError::Invalid { field: "timeout_ms".into(), reason: "must be at least 100ms".into() });
        }
        if self.timeout_ms > 300_000 {
            return Err(ConfigError::Invalid {
                field: "timeout_ms".into(),
                reason: "must not exceed 5 minutes (300000ms)".into(),
            });
        }

        if self.user_agent.is_empty() {
            return Err(ConfigError::Invalid { field: "user_agent".into(), reason: "must not be empty".into() });
        }

        if !self.allowlist_domains.is_empty() && !self.denylist_domains.is_empty() {
            tracing::warn!(
                allowlist_count = self.allowlist_domains.len(),
                denylist_count = self.denylist_domains.len(),
                "Both allowlist_domains and denylist_domains are set; \
                 allowlist takes precedence"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_default_config() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_max_bytes_zero() {
        let config = AppConfig { max_bytes: 0, ..Default::default() };
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::Invalid { field, .. }) if field == "max_bytes"));
    }

    #[test]
    fn test_validate_max_bytes_exceeds_limit() {
        let config = AppConfig { max_bytes: 51 * 1024 * 1024, ..Default::default() }; // 51MB
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::Invalid { field, .. }) if field == "max_bytes"));
    }

    #[test]
    fn test_validate_timeout_too_small() {
        let config = AppConfig { timeout_ms: 50, ..Default::default() };
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::Invalid { field, .. }) if field == "timeout_ms"));
    }

    #[test]
    fn test_validate_timeout_exceeds_limit() {
        let config = AppConfig { timeout_ms: 301_000, ..Default::default() }; // 5min 1sec
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::Invalid { field, .. }) if field == "timeout_ms"));
    }

    #[test]
    fn test_validate_empty_user_agent() {
        let config = AppConfig { user_agent: String::new(), ..Default::default() };
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::Invalid { field, .. }) if field == "user_agent"));
    }

    #[test]
    fn test_validate_edge_case_values() {
        let config = AppConfig { max_bytes: 1, timeout_ms: 100, ..Default::default() }; // minimum valid values
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_max_values() {
        let config = AppConfig { max_bytes: 50 * 1024 * 1024, timeout_ms: 300_000, ..Default::default() }; // exactly 50MB
        assert!(config.validate().is_ok());
    }
}
