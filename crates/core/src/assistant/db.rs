//! Database connection pool + refinery migration runner.

use anyhow::Result;
use refinery::embed_migrations;
use rusqlite::Connection;
use std::path::Path;

embed_migrations!("migrations");

/// Initialise a SQLite connection at the given path and run all pending migrations.
///
/// Returns an open connection ready for data-access functions to use.
pub fn init(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrations::runner().run(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_creates_db_file_and_runs_migrations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");

        let conn = init(&path).expect("init should succeed");

        // Migrations ran — the three tables should exist.
        for table in [
            "conversation",
            "message",
            "proposal",
            "task",
            "calendar_account",
            "event",
            "person",
            "chore",
            "chore_completion",
            "rotation",
            "time_block",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "table {table} should exist after migrations");
        }

        assert!(path.exists(), "db file should exist on disk");
    }

    #[test]
    fn init_is_idempotent_on_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        {
            let _c = init(&path).unwrap();
        }
        let _c = init(&path).expect("second init should succeed");
    }
}
