//! Database schema migrations.
//!
//! Uses a simple version table approach to track applied migrations.
//! Each migration is a SQL batch that transforms the schema.

use std::num::ParseIntError;

use super::Error;
use tokio_rusqlite::{Connection, params};

/// Migration list: (version, description, SQL).
///
/// Migrations must be applied in order. The version number is an
/// incrementing integer used to track which migrations have been applied.
/// All migrations are idempotent using CREATE IF NOT EXISTS.
const MIGRATIONS: &[(&str, &str)] = &[
    ("1", include_str!("../../migrations/001_snapshots.sql")),
    ("2", include_str!("../../migrations/002_search_cache.sql")),
];

/// Run any pending migrations.
///
/// This creates the _migrations table if it doesn't exist, checks the
/// current version, and applies any migrations that haven't been run yet.
///
/// # Arguments
///
/// * `conn` - Database connection
///
/// # Errors
///
/// Returns an error if a migration SQL fails to execute.
pub async fn run(conn: &Connection) -> Result<(), Error> {
    conn.call(|conn| -> Result<(), Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            )",
            [],
        )
        .map_err(Error::from)?;

        let current: i64 = conn
            .query_row("SELECT COALESCE(MAX(version), 0) FROM _migrations", [], |row| {
                row.get(0)
            })
            .map_err(Error::from)?;

        for (version, sql) in MIGRATIONS {
            let version_num: i64 = version
                .parse()
                .map_err(|e: ParseIntError| Error::MigrationFailed(e.to_string()))?;
            if version_num > current {
                conn.execute_batch(sql)?;
                conn.execute(
                    "INSERT INTO _migrations (version, applied_at) VALUES (?1, ?2)",
                    params![version_num, chrono::Utc::now().to_rfc3339()],
                )
                .map_err(Error::from)?;
            }
        }

        Ok(())
    })
    .await
    .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let conn = Connection::open_in_memory().await.unwrap();
        run(&conn).await.unwrap();
        run(&conn).await.unwrap();

        let has_snapshots: bool = conn
            .call(|conn| {
                conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='snapshots')",
                    [],
                    |row| row.get(0),
                )
            })
            .await
            .unwrap();

        assert!(has_snapshots);
    }

    #[tokio::test]
    async fn test_migrations_version_tracking() {
        let conn = Connection::open_in_memory().await.unwrap();
        run(&conn).await.unwrap();

        let count: i64 = conn
            .call(|conn| conn.query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0)))
            .await
            .unwrap();

        assert_eq!(count, MIGRATIONS.len() as i64);
    }
}
