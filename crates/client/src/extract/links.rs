//! Link harvesting and URL fixing from HTML documents.

use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

/// A harvested link with text and href.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Link {
    /// Link text content
    pub text: String,
    /// Resolved href URL
    pub href: String,
}

/// Extract links from an HTML document, resolving relative URLs against the base URL.
///
/// This extracts all `<a>` tags with href attributes, resolves relative URLs,
/// and removes duplicates (by href).
pub fn extract_links(html: &str, base_url: &Url) -> Vec<Link> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").expect("invalid selector");

    let mut seen = HashSet::new();
    let mut links = Vec::new();

    for element in document.select(&selector) {
        let href = match element.value().attr("href") {
            Some(h) => h.to_string(),
            None => continue,
        };

        let resolved = match base_url.join(&href) {
            Ok(u) => u.to_string(),
            Err(_) => continue,
        };

        if seen.contains(&resolved) {
            continue;
        }

        seen.insert(resolved.clone());

        let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
        let text = if text.is_empty() { "[link]".to_string() } else { text };

        links.push(Link { text, href: resolved });
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links_basic() {
        let html = r#"
            <html>
                <body>
                    <a href="https://example.com">Example</a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "Example");
        assert_eq!(links[0].href, "https://example.com/");
    }

    #[test]
    fn test_extract_links_relative() {
        let html = r#"
            <html>
                <body>
                    <a href="/about">About</a>
                    <a href="contact">Contact</a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com/path/").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].text, "About");
        assert_eq!(links[0].href, "https://example.com/about");
        assert_eq!(links[1].text, "Contact");
        assert_eq!(links[1].href, "https://example.com/path/contact");
    }

    #[test]
    fn test_extract_links_duplicate() {
        let html = r#"
            <html>
                <body>
                    <a href="https://example.com">First</a>
                    <a href="https://example.com">Second</a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "First");
    }

    #[test]
    fn test_extract_links_empty_text() {
        let html = r#"
            <html>
                <body>
                    <a href="https://example.com"></a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "[link]");
    }

    #[test]
    fn test_extract_links_whitespace_text() {
        let html = r#"
            <html>
                <body>
                    <a href="https://example.com">   </a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "[link]");
    }

    #[test]
    fn test_extract_links_multiline_text() {
        let html = r#"
            <html>
                <body>
                    <a href="https://example.com">
                        Line 1
                        Line 2
                    </a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 1);
        assert!(links[0].text.contains("Line 1"));
        assert!(links[0].text.contains("Line 2"));
    }

    #[test]
    fn test_extract_links_invalid_url() {
        let html = r#"
            <html>
                <body>
                    <a href=":not-a-url">Invalid</a>
                    <a href="https://example.com">Valid</a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn test_extract_links_no_links() {
        let html = r#"
            <html>
                <body>
                    <p>No links here</p>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_links_fragment() {
        let html = r##"
            <html>
                <body>
                    <a href="#section">Section</a>
                </body>
            </html>
        "##;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].href, "https://example.com/#section");
    }

    #[test]
    fn test_extract_links_query() {
        let html = r#"
            <html>
                <body>
                    <a href="/search?q=test">Search</a>
                </body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let links = extract_links(html, &base);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].href, "https://example.com/search?q=test");
    }
}
