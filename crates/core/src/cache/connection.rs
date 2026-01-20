//! Database connection management with pragma configuration.
//!
//! This module handles opening the SQLite database, applying required pragmas
//! for performance and concurrency (WAL mode), and running migrations.

use super::migrations;
use crate::Error;
use std::path::Path;
use tokio_rusqlite::Connection;

/// Cache database handle.
///
/// Wraps a tokio-rusqlite Connection that runs database operations
/// on a background thread.
#[derive(Clone, Debug)]
pub struct CacheDb {
    pub(crate) conn: Connection,
}

impl CacheDb {
    /// Open a database at the specified path.
    ///
    /// Creates the file if it doesn't exist, applies performance pragmas,
    /// and runs any pending migrations.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let conn = Connection::open(path).await.map_err(|e| Error::Database(e.into()))?;

        conn.call(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=NORMAL;
                 PRAGMA temp_store=MEMORY;
                 PRAGMA foreign_keys=ON;",
            )?;
            Ok(())
        })
        .await
        .map_err(Error::Database)?;

        migrations::run(&conn).await?;

        Ok(Self { conn })
    }

    /// Open an in-memory database for testing.
    ///
    /// Creates a temporary in-memory SQLite database with the same
    /// pragma configuration as file-based databases.
    pub async fn open_in_memory() -> Result<Self, Error> {
        let conn = Connection::open_in_memory()
            .await
            .map_err(|e| Error::Database(e.into()))?;

        conn.call(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=NORMAL;
                 PRAGMA temp_store=MEMORY;
                 PRAGMA foreign_keys=ON;",
            )?;
            Ok(())
        })
        .await
        .map_err(Error::Database)?;

        migrations::run(&conn).await?;

        Ok(Self { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_in_memory() {
        let db = CacheDb::open_in_memory().await.unwrap();
        let version = db
            .conn
            .call(|conn| conn.query_row("SELECT sqlite_version()", [], |row| row.get::<_, String>(0)))
            .await
            .unwrap();
        assert!(!version.is_empty());
    }
}
