//! Application configuration with layered loading.
//!
//! This module provides configuration management using figment for layered
//! configuration loading from multiple sources:
//!
//! 1. Environment variables (MCP_WEB_*)
//! 2. TOML config file (if MCP_WEB_CONFIG_FILE set)
//! 3. Built-in defaults

use std::path::PathBuf;
use std::time::Duration;

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

mod validation;

pub use validation::ConfigError;

/// Application configuration with layered loading.
///
/// Loading precedence (highest wins):
/// 1. Environment variables (MCP_WEB_*)
/// 2. TOML config file (if MCP_WEB_CONFIG_FILE set)
/// 3. Built-in defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Brave API subscription token for web_search.
    ///
    /// Set via MCP_WEB_BRAVE_API_KEY environment variable.
    /// Required only when web_search tool is called.
    #[serde(default)]
    pub brave_api_key: Option<String>,

    /// Path to SQLite cache database.
    ///
    /// Set via MCP_WEB_DB_PATH environment variable.
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,

    /// User-Agent string for HTTP requests.
    ///
    /// Set via MCP_WEB_USER_AGENT environment variable.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,

    /// Maximum bytes to fetch per request.
    ///
    /// Set via MCP_WEB_MAX_BYTES environment variable.
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,

    /// HTTP request timeout in milliseconds.
    ///
    /// Set via MCP_WEB_TIMEOUT_MS environment variable.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Whether to respect robots.txt rules.
    ///
    /// Set via MCP_WEB_RESPECT_ROBOTS environment variable.
    #[serde(default = "default_true")]
    pub respect_robots: bool,

    /// Whether rendered mode (headless browser) is enabled.
    ///
    /// Set via MCP_WEB_RENDER_ENABLED environment variable.
    #[serde(default)]
    pub render_enabled: bool,

    /// Domain allowlist for fetch operations.
    ///
    /// Set via MCP_WEB_ALLOWLIST_DOMAINS environment variable (comma-separated).
    #[serde(default)]
    pub allowlist_domains: Vec<String>,

    /// Domain denylist for fetch operations.
    ///
    /// Set via MCP_WEB_DENYLIST_DOMAINS environment variable (comma-separated).
    #[serde(default)]
    pub denylist_domains: Vec<String>,
}

fn default_db_path() -> PathBuf {
    PathBuf::from("./mcp-web-cache.sqlite")
}

fn default_user_agent() -> String {
    "mcp-web/0.1".into()
}

fn default_max_bytes() -> usize {
    5_242_880 // 5MB
}

fn default_timeout_ms() -> u64 {
    20_000
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            brave_api_key: None,
            db_path: default_db_path(),
            user_agent: default_user_agent(),
            max_bytes: default_max_bytes(),
            timeout_ms: default_timeout_ms(),
            respect_robots: true,
            render_enabled: false,
            allowlist_domains: Vec::new(),
            denylist_domains: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Timeout as Duration for use with reqwest/tokio.
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    /// Load configuration from all sources with layered precedence.
    ///
    /// Priority (highest wins):
    /// 1. Environment variables prefixed with `MCP_WEB_`
    /// 2. TOML file from `MCP_WEB_CONFIG_FILE` (if set)
    /// 3. Built-in defaults via `Default::default()`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if:
    /// - Configuration file cannot be read
    /// - Environment variables cannot be parsed
    /// - Validation fails after loading
    pub fn load() -> Result<Self, ConfigError> {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));

        if let Ok(config_path) = std::env::var("MCP_WEB_CONFIG_FILE") {
            figment = figment.merge(Toml::file(&config_path));
        }

        figment = figment.merge(
            Env::prefixed("MCP_WEB_")
                .map(|key| key.as_str().to_lowercase().into())
                .split("__"),
        );

        let config: Self = figment.extract().map_err(|e| ConfigError::LoadFailed(e.to_string()))?;

        config.validate()?;

        Ok(config)
    }

    /// Check if Brave API key is available (for deferred validation).
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::Missing` if the Brave API key is not set.
    pub fn require_brave_api_key(&self) -> Result<&str, ConfigError> {
        self.brave_api_key.as_deref().ok_or_else(|| ConfigError::Missing {
            field: "brave_api_key".into(),
            hint: "Set MCP_WEB_BRAVE_API_KEY environment variable".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.db_path, PathBuf::from("./mcp-web-cache.sqlite"));
        assert_eq!(config.user_agent, "mcp-web/0.1");
        assert_eq!(config.max_bytes, 5_242_880);
        assert_eq!(config.timeout_ms, 20_000);
        assert!(config.respect_robots);
        assert!(!config.render_enabled);
        assert!(config.allowlist_domains.is_empty());
        assert!(config.denylist_domains.is_empty());
        assert!(config.brave_api_key.is_none());
    }

    #[test]
    fn test_timeout_duration() {
        let config = AppConfig::default();
        assert_eq!(config.timeout(), Duration::from_millis(20_000));
    }

    #[test]
    fn test_require_brave_api_key_missing() {
        let config = AppConfig::default();
        let result = config.require_brave_api_key();
        assert!(matches!(result, Err(ConfigError::Missing { .. })));
    }

    #[test]
    fn test_require_brave_api_key_present() {
        let config = AppConfig { brave_api_key: Some("test-key".into()), ..Default::default() };
        let result = config.require_brave_api_key();
        assert_eq!(result.unwrap(), "test-key");
    }
}
