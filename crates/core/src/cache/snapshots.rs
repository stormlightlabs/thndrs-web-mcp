//! Snapshot CRUD operations.
//!
//! Provides functions for creating, reading, updating, and deleting
//! cached document snapshots.

use super::connection::CacheDb;
use crate::Error;
use serde::{Deserialize, Serialize};
use tokio_rusqlite::params;
use tokio_rusqlite::rusqlite;

/// A cached document snapshot.
///
/// Represents a fetched and extracted web page, with all metadata
/// needed for cache invalidation and reproducibility.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Snapshot {
    pub hash: String,
    pub url: String,
    pub final_url: String,
    pub mode: String,
    pub content_type: Option<String>,
    pub status_code: Option<i32>,
    pub fetched_at: String,
    pub expires_at: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,

    // TODO: ContentContext struct
    pub raw_bytes: Option<Vec<u8>>,
    pub raw_truncated: bool,
    pub title: Option<String>,
    pub markdown: Option<String>,
    pub text: Option<String>,
    pub links_json: Option<String>,

    // TODO: ExtractorContext struct
    pub extractor_name: Option<String>,
    pub extractor_version: Option<String>,
    pub siteconfig_id: Option<String>,
    pub extract_cfg_json: Option<String>,

    // TODO: DebugContext struct
    pub headers_json: Option<String>,
    pub fetch_ms: Option<i64>,
    pub extract_ms: Option<i64>,
}

impl CacheDb {
    /// Insert or update a cached snapshot.
    ///
    /// Uses UPSERT semantics: inserts if the hash doesn't exist,
    /// updates all fields if it does.
    pub async fn upsert_snapshot(&self, snapshot: &Snapshot) -> Result<(), Error> {
        let snapshot = snapshot.clone();
        self.conn
            .call(move |conn| -> Result<(), Error> {
                conn.execute(
                    "INSERT INTO snapshots (
                    hash, url, final_url, mode, content_type, status_code,
                    fetched_at, expires_at, etag, last_modified,
                    raw_bytes, raw_truncated, title, markdown, text, links_json,
                    extractor_name, extractor_version, siteconfig_id, extract_cfg_json,
                    headers_json, fetch_ms, extract_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                          ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                          ?21, ?22, ?23)
                ON CONFLICT(hash) DO UPDATE SET
                    url = excluded.url,
                    final_url = excluded.final_url,
                    mode = excluded.mode,
                    content_type = excluded.content_type,
                    status_code = excluded.status_code,
                    fetched_at = excluded.fetched_at,
                    expires_at = excluded.expires_at,
                    etag = excluded.etag,
                    last_modified = excluded.last_modified,
                    raw_bytes = excluded.raw_bytes,
                    raw_truncated = excluded.raw_truncated,
                    title = excluded.title,
                    markdown = excluded.markdown,
                    text = excluded.text,
                    links_json = excluded.links_json,
                    extractor_name = excluded.extractor_name,
                    extractor_version = excluded.extractor_version,
                    siteconfig_id = excluded.siteconfig_id,
                    extract_cfg_json = excluded.extract_cfg_json,
                    headers_json = excluded.headers_json,
                    fetch_ms = excluded.fetch_ms,
                    extract_ms = excluded.extract_ms",
                    params![
                        &snapshot.hash,
                        &snapshot.url,
                        &snapshot.final_url,
                        &snapshot.mode,
                        &snapshot.content_type,
                        &snapshot.status_code,
                        &snapshot.fetched_at,
                        &snapshot.expires_at,
                        &snapshot.etag,
                        &snapshot.last_modified,
                        &snapshot.raw_bytes,
                        snapshot.raw_truncated as i32,
                        &snapshot.title,
                        &snapshot.markdown,
                        &snapshot.text,
                        &snapshot.links_json,
                        &snapshot.extractor_name,
                        &snapshot.extractor_version,
                        &snapshot.siteconfig_id,
                        &snapshot.extract_cfg_json,
                        &snapshot.headers_json,
                        &snapshot.fetch_ms,
                        &snapshot.extract_ms,
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(Error::from)
    }

    /// Get a snapshot by hash.
    ///
    /// Returns None if the hash doesn't exist in the cache.
    pub async fn get_snapshot(&self, hash: &str) -> Result<Option<Snapshot>, Error> {
        let hash = hash.to_string();
        self.conn
            .call(move |conn| -> Result<Option<Snapshot>, Error> {
                let mut stmt = conn.prepare(
                    "SELECT
                    hash, url, final_url, mode, content_type, status_code,
                    fetched_at, expires_at, etag, last_modified,
                    raw_bytes, raw_truncated, title, markdown, text, links_json,
                    extractor_name, extractor_version, siteconfig_id, extract_cfg_json,
                    headers_json, fetch_ms, extract_ms
                FROM snapshots WHERE hash = ?1",
                )?;

                let result = stmt.query_row(params![hash], |row| {
                    Ok(Snapshot {
                        hash: row.get(0)?,
                        url: row.get(1)?,
                        final_url: row.get(2)?,
                        mode: row.get(3)?,
                        content_type: row.get(4)?,
                        status_code: row.get(5)?,
                        fetched_at: row.get(6)?,
                        expires_at: row.get(7)?,
                        etag: row.get(8)?,
                        last_modified: row.get(9)?,
                        raw_bytes: row.get(10)?,
                        raw_truncated: row.get::<_, i32>(11)? == 1,
                        title: row.get(12)?,
                        markdown: row.get(13)?,
                        text: row.get(14)?,
                        links_json: row.get(15)?,
                        extractor_name: row.get(16)?,
                        extractor_version: row.get(17)?,
                        siteconfig_id: row.get(18)?,
                        extract_cfg_json: row.get(19)?,
                        headers_json: row.get(20)?,
                        fetch_ms: row.get(21)?,
                        extract_ms: row.get(22)?,
                    })
                });

                match result {
                    Ok(s) => Ok(Some(s)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e.into()),
                }
            })
            .await
            .map_err(Error::from)
    }

    /// Check if a snapshot exists and is fresh.
    ///
    /// Returns false if the snapshot doesn't exist or has expired.
    pub async fn is_snapshot_fresh(&self, hash: &str) -> Result<bool, Error> {
        let hash = hash.to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .call(move |conn| -> Result<bool, Error> {
                let fresh: bool = conn
                    .query_row(
                        "SELECT EXISTS(
                    SELECT 1 FROM snapshots
                    WHERE hash = ?1
                    AND (expires_at IS NULL OR expires_at > ?2)
                )",
                        params![hash, now],
                        |row| row.get(0),
                    )
                    .map_err(Error::from)?;

                Ok(fresh)
            })
            .await
            .map_err(Error::from)
    }

    /// Delete expired snapshots.
    ///
    /// Returns the number of deleted entries.
    pub async fn purge_expired_snapshots(&self) -> Result<u64, Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .call(move |conn| -> Result<u64, Error> {
                let count = conn.execute(
                    "DELETE FROM snapshots WHERE expires_at IS NOT NULL AND expires_at < ?1",
                    params![now],
                )?;
                Ok(count as u64)
            })
            .await
            .map_err(Error::from)
    }

    /// Delete snapshots by domain pattern.
    ///
    /// Returns the number of deleted entries.
    pub async fn purge_snapshots_by_domain(&self, domain: &str) -> Result<u64, Error> {
        let pattern = format!("%{domain}%");
        self.conn
            .call(move |conn| -> Result<u64, Error> {
                let count = conn.execute("DELETE FROM snapshots WHERE url LIKE ?1", params![pattern])?;
                Ok(count as u64)
            })
            .await
            .map_err(Error::from)
    }

    /// Purge oldest entries until count <= max_entries.
    ///
    /// Returns the number of deleted entries.
    pub async fn purge_lru_snapshots(&self, max_entries: usize) -> Result<u64, Error> {
        let max = max_entries as i64;
        self.conn
            .call(move |conn| -> Result<u64, Error> {
                let count: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))?;
                if count <= max {
                    return Ok(0);
                }

                let to_delete = count - max;
                let deleted = conn.execute(
                    "DELETE FROM snapshots WHERE hash IN (
                    SELECT hash FROM snapshots ORDER BY fetched_at ASC LIMIT ?1
                )",
                    params![to_delete],
                )?;
                Ok(deleted as u64)
            })
            .await
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::hash::compute_cache_key;

    fn make_test_snapshot(url: &str) -> Snapshot {
        let hash = compute_cache_key(url, "", "readable");
        Snapshot {
            hash,
            url: url.to_string(),
            final_url: url.to_string(),
            mode: "readable".to_string(),
            content_type: Some("text/html".to_string()),
            status_code: Some(200),
            fetched_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
            etag: None,
            last_modified: None,
            raw_bytes: None,
            raw_truncated: false,
            title: Some("Test".to_string()),
            markdown: Some("# Test".to_string()),
            text: Some("Test".to_string()),
            links_json: None,
            extractor_name: Some("lectito-core".to_string()),
            extractor_version: Some("0.1.0".to_string()),
            siteconfig_id: None,
            extract_cfg_json: None,
            headers_json: None,
            fetch_ms: Some(100),
            extract_ms: Some(50),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        let snapshot = make_test_snapshot("https://example.com");

        db.upsert_snapshot(&snapshot).await.unwrap();

        let retrieved = db.get_snapshot(&snapshot.hash).await.unwrap().unwrap();
        assert_eq!(retrieved.url, snapshot.url);
        assert_eq!(retrieved.title, snapshot.title);
    }

    #[tokio::test]
    async fn test_get_missing() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        let result = db.get_snapshot("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_purge_by_domain() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        db.upsert_snapshot(&make_test_snapshot("https://example.com/page1"))
            .await
            .unwrap();
        db.upsert_snapshot(&make_test_snapshot("https://other.com/page2"))
            .await
            .unwrap();

        let deleted = db.purge_snapshots_by_domain("example.com").await.unwrap();
        assert_eq!(deleted, 1);

        let remaining = db
            .get_snapshot(&compute_cache_key("https://example.com/page1", "", "readable"))
            .await
            .unwrap();
        assert!(remaining.is_none());

        let other = db
            .get_snapshot(&compute_cache_key("https://other.com/page2", "", "readable"))
            .await
            .unwrap();
        assert!(other.is_some());
    }
}
