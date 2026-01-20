//! Headless browser rendering for JS-heavy pages.
//!
//! This module provides a feature-gated renderer trait and implementation
//! using chromiumoxide for headless Chrome/Chromium browser control.

use std::time::Duration;
use thiserror::Error;
use url::Url;

/// Errors that can occur during page rendering.
#[derive(Debug, Error)]
pub enum RenderError {
    /// Failed to launch or connect to browser.
    #[error("browser launch failed: {0}")]
    BrowserLaunch(String),

    /// Failed to navigate to URL.
    #[error("navigation failed: {0}")]
    Navigation(String),

    /// Failed to get page content.
    #[error("content retrieval failed: {0}")]
    ContentRetrieval(String),

    /// Timeout waiting for page to load.
    #[error("render timeout after {0}ms")]
    Timeout(u64),

    /// Wait selector not found.
    #[error("wait_for selector not found: {0}")]
    SelectorNotFound(String),

    /// Browser closed unexpectedly.
    #[error("browser closed unexpectedly")]
    BrowserClosed,
}

/// Options for rendering a page.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Timeout in milliseconds (default: 30000).
    pub timeout_ms: u64,

    /// Optional CSS selector to wait for before extracting content.
    pub wait_for: Option<String>,

    /// Viewport dimensions (default: 1280x720).
    pub viewport: (u32, u32),
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { timeout_ms: 30000, wait_for: None, viewport: (1280, 720) }
    }
}

/// Result of rendering a page.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    /// Rendered HTML content.
    pub html: String,

    /// Final URL after redirects.
    pub final_url: Url,

    /// Time taken to render in milliseconds.
    pub render_time_ms: u64,
}

/// Renderer trait for headless browser page rendering.
#[async_trait::async_trait]
pub trait Renderer: Send + Sync {
    /// Render a URL to HTML via headless browser.
    async fn render(&self, url: &Url, opts: &RenderOptions) -> Result<RenderedPage, RenderError>;
}

/// Headless Chrome/Chromium renderer using chromiumoxide.
pub struct HeadlessRenderer {
    _browser: chromiumoxide::Browser,
}

impl HeadlessRenderer {
    /// Create a new headless renderer by launching a browser instance.
    ///
    /// The browser runs in headless mode and uses a background task
    /// to handle Chrome DevTools Protocol events.
    pub async fn new() -> Result<Self, RenderError> {
        use chromiumoxide::browser::{Browser, BrowserConfig};
        use futures_util::StreamExt;

        let (browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .with_head()
                .build()
                .map_err(RenderError::BrowserLaunch)?,
        )
        .await
        .map_err(|e| RenderError::BrowserLaunch(e.to_string()))?;

        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(e) = event {
                    tracing::debug!("browser handler event error: {e}");
                    break;
                }
            }
        });

        Ok(Self { _browser: browser })
    }
}

#[async_trait::async_trait]
impl Renderer for HeadlessRenderer {
    async fn render(&self, url: &Url, opts: &RenderOptions) -> Result<RenderedPage, RenderError> {
        let page = self
            ._browser
            .new_page(url.as_str())
            .await
            .map_err(|e| RenderError::Navigation(e.to_string()))?;

        let start = std::time::Instant::now();

        if let Some(selector) = &opts.wait_for {
            let wait_result = tokio::time::timeout(Duration::from_millis(opts.timeout_ms), async {
                for _ in 0..30 {
                    if (page.find_element(selector).await).is_ok() {
                        return Ok::<(), RenderError>(());
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(RenderError::SelectorNotFound(selector.clone()))
            })
            .await;

            match wait_result {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    return Err(e);
                }
                Err(_) => {
                    return Err(RenderError::Timeout(opts.timeout_ms));
                }
            }
        } else {
            tokio::time::timeout(Duration::from_millis(opts.timeout_ms), async {
                tokio::time::sleep(Duration::from_millis(2000)).await;
            })
            .await
            .map_err(|_| RenderError::Timeout(opts.timeout_ms))?;
        }

        let html = page
            .content()
            .await
            .map_err(|e| RenderError::ContentRetrieval(e.to_string()))?;

        let page_url = page
            .url()
            .await
            .map_err(|e| RenderError::ContentRetrieval(e.to_string()))?;

        let final_url = Url::parse(page_url.as_deref().unwrap_or(url.as_str()))
            .map_err(|e| RenderError::Navigation(e.to_string()))?;

        let render_time_ms = start.elapsed().as_millis() as u64;

        page.close().await.ok();
        Ok(RenderedPage { html, final_url, render_time_ms })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Chrome/Chromium installation"]
    async fn test_headless_renderer_new() {
        let renderer = HeadlessRenderer::new().await;
        assert!(renderer.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires network and Chrome/Chromium"]
    async fn test_render_simple_page() {
        let renderer = HeadlessRenderer::new().await.unwrap();
        let url = Url::parse("https://example.com").unwrap();
        let opts = RenderOptions::default();

        let result = renderer.render(&url, &opts).await;
        assert!(result.is_ok());

        let page = result.unwrap();
        assert!(page.html.contains("<html>"));
        assert_eq!(page.final_url.as_str(), "https://example.com/");
    }
}
