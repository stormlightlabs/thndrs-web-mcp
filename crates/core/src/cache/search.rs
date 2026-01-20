//! Search cache operations.
//!
//! Provides functions for caching and retrieving Brave Search API results.

use super::connection::CacheDb;
use crate::Error;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio_rusqlite::params;

/// Cached search result metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCacheMeta {
    pub query_json: String,
    pub fetched_at: String,
    pub expires_at: String,
}

impl CacheDb {
    /// Get a cached search response by key hash.
    ///
    /// Returns None if the key doesn't exist in the cache.
    pub async fn get_search(&self, key_hash: &str) -> Result<Option<String>, Error> {
        let key_hash = key_hash.to_string();
        self.conn
            .call(move |conn| -> Result<Option<String>, Error> {
                let mut stmt = conn.prepare("SELECT response_json FROM search_cache WHERE key_hash = ?1")?;

                let result = stmt.query_row(params![key_hash], |row| row.get(0));

                match result {
                    Ok(json) => Ok(Some(json)),
                    Err(tokio_rusqlite::rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e.into()),
                }
            })
            .await
            .map_err(Error::from)
    }

    /// Get search cache metadata by key hash.
    pub async fn get_search_meta(&self, key_hash: &str) -> Result<Option<SearchCacheMeta>, Error> {
        let key_hash = key_hash.to_string();
        self.conn
            .call(move |conn| -> Result<Option<SearchCacheMeta>, Error> {
                let mut stmt =
                    conn.prepare("SELECT query_json, fetched_at, expires_at FROM search_cache WHERE key_hash = ?1")?;

                let result = stmt.query_row(params![key_hash], |row| {
                    Ok(SearchCacheMeta { query_json: row.get(0)?, fetched_at: row.get(1)?, expires_at: row.get(2)? })
                });

                match result {
                    Ok(meta) => Ok(Some(meta)),
                    Err(tokio_rusqlite::rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e.into()),
                }
            })
            .await
            .map_err(Error::from)
    }

    /// Check if a search cache entry exists and is fresh.
    ///
    /// Returns false if the entry doesn't exist or has expired.
    pub async fn is_search_fresh(&self, key_hash: &str) -> Result<bool, Error> {
        let key_hash = key_hash.to_string();
        let now = Utc::now().to_rfc3339();
        self.conn
            .call(move |conn| -> Result<bool, Error> {
                let fresh: bool = conn
                    .query_row(
                        "SELECT EXISTS(
                        SELECT 1 FROM search_cache
                        WHERE key_hash = ?1
                        AND expires_at > ?2
                    )",
                        params![key_hash, now],
                        |row| row.get(0),
                    )
                    .map_err(Error::from)?;

                Ok(fresh)
            })
            .await
            .map_err(Error::from)
    }

    /// Insert or update a cached search result.
    ///
    /// Uses UPSERT semantics: inserts if the key doesn't exist, updates all fields if it does.
    pub async fn put_search(
        &self, key_hash: &str, query_json: &str, response_json: &str, ttl_seconds: i64,
    ) -> Result<(), Error> {
        let key_hash = key_hash.to_string();
        let query_json = query_json.to_string();
        let response_json = response_json.to_string();

        let fetched_at = Utc::now().to_rfc3339();
        let expires_at = (Utc::now() + Duration::seconds(ttl_seconds)).to_rfc3339();

        self.conn
            .call(move |conn| -> Result<(), Error> {
                conn.execute(
                    "INSERT INTO search_cache (key_hash, query_json, response_json, fetched_at, expires_at)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(key_hash) DO UPDATE SET
                        query_json = excluded.query_json,
                        response_json = excluded.response_json,
                        fetched_at = excluded.fetched_at,
                        expires_at = excluded.expires_at",
                    params![key_hash, query_json, response_json, fetched_at, expires_at],
                )?;
                Ok(())
            })
            .await
            .map_err(Error::from)
    }

    /// Delete expired search cache entries.
    ///
    /// Returns the number of deleted entries.
    pub async fn purge_expired_search(&self) -> Result<u64, Error> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .call(move |conn| -> Result<u64, Error> {
                let count = conn.execute("DELETE FROM search_cache WHERE expires_at < ?1", params![now])?;
                Ok(count as u64)
            })
            .await
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_put_and_get_search() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        let key = "test_key_hash";
        let query_json = r#"{"q":"test","count":10}"#;
        let response_json = r#"{"results":[],"query":{"original":"test"}}"#;

        db.put_search(key, query_json, response_json, 3600).await.unwrap();

        let retrieved = db.get_search(key).await.unwrap().unwrap();
        assert_eq!(retrieved, response_json);
    }

    #[tokio::test]
    async fn test_get_missing_search() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        let result = db.get_search("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_search_freshness() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        let key = "test_freshness";
        assert!(!db.is_search_fresh(key).await.unwrap());

        db.put_search(key, "{}", "{}", 1).await.unwrap();

        assert!(db.is_search_fresh(key).await.unwrap());
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        assert!(!db.is_search_fresh(key).await.unwrap());
    }

    #[tokio::test]
    async fn test_purge_expired_search() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        db.put_search("expiring", "{}", "{}", 1).await.unwrap();
        db.put_search("fresh", "{}", "{}", 3600).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let deleted = db.purge_expired_search().await.unwrap();
        assert_eq!(deleted, 1);
        assert!(db.get_search("expiring").await.unwrap().is_none());
        assert!(db.get_search("fresh").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_upsert_search() {
        let db = super::super::connection::CacheDb::open_in_memory().await.unwrap();
        let key = "upsert_test";

        db.put_search(key, r#"{"old":1}"#, r#"{"old":1}"#, 3600).await.unwrap();
        db.put_search(key, r#"{"new":2}"#, r#"{"new":2}"#, 3600).await.unwrap();

        let retrieved = db.get_search(key).await.unwrap().unwrap();
        assert_eq!(retrieved, r#"{"new":2}"#);
    }
}
