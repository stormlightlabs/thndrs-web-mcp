-- Migration 2: Create search_cache table
-- Stores cached Brave Search API results with short TTL
-- This migration is idempotent: using CREATE TABLE IF NOT EXISTS

CREATE TABLE IF NOT EXISTS search_cache (
    key_hash        TEXT PRIMARY KEY,
    query_json      TEXT NOT NULL,
    response_json   TEXT NOT NULL,
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT NOT NULL
);
