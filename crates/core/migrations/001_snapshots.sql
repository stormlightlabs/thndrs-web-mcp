-- Migration 1: Create snapshots table and indexes
-- Stores cached document snapshots with metadata and extracted content
-- This migration is idempotent: using CREATE TABLE IF NOT EXISTS and CREATE INDEX IF NOT EXISTS

CREATE TABLE IF NOT EXISTS snapshots (
    hash            TEXT PRIMARY KEY,
    url             TEXT NOT NULL,
    final_url       TEXT NOT NULL,
    mode            TEXT NOT NULL,
    content_type    TEXT,
    status_code     INTEGER,
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT,
    etag            TEXT,
    last_modified   TEXT,
    raw_bytes       BLOB,
    raw_truncated   INTEGER NOT NULL DEFAULT 0,
    title           TEXT,
    markdown        TEXT,
    text            TEXT,
    links_json      TEXT,
    extractor_name      TEXT,
    extractor_version   TEXT,
    siteconfig_id       TEXT,
    extract_cfg_json    TEXT,
    headers_json    TEXT,
    fetch_ms        INTEGER,
    extract_ms      INTEGER
);

CREATE INDEX IF NOT EXISTS idx_snapshots_url ON snapshots(url);
CREATE INDEX IF NOT EXISTS idx_snapshots_fetched ON snapshots(fetched_at);
CREATE INDEX IF NOT EXISTS idx_snapshots_expires ON snapshots(expires_at);
