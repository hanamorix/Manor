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

        // Migrations ran — all expected tables should exist.
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

    #[test]
    fn post_migrations_state_has_no_gocardless_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_v23.db");

        let conn = init(&path).expect("init should succeed");

        for table in ["bank_account", "gocardless_institution_cache"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                count, 0,
                "table {table} should NOT exist after V23 migration"
            );
        }
    }

    #[test]
    fn drop_bank_sync_relabels_source_sync_rows() {
        // Simulate post-V22 state with a plain in-memory connection (no refinery).
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        // Mirror the post-V22 schema so V23's table rebuild can SELECT the
        // columns it expects. This is the minimum set of columns the rebuild
        // copies over, including the UNIQUE and indexes V23 has to drop.
        conn.execute_batch(
            "CREATE TABLE category (id INTEGER PRIMARY KEY);
             CREATE TABLE bank_account (id INTEGER PRIMARY KEY);
             CREATE TABLE ledger_transaction (
                id                    INTEGER PRIMARY KEY,
                bank_account_id       INTEGER REFERENCES bank_account(id),
                external_id           TEXT,
                amount_pence          INTEGER NOT NULL,
                currency              TEXT    NOT NULL DEFAULT 'GBP',
                description           TEXT    NOT NULL,
                merchant              TEXT,
                category_id           INTEGER REFERENCES category(id),
                date                  INTEGER NOT NULL,
                source                TEXT    NOT NULL DEFAULT 'manual',
                note                  TEXT,
                created_at            INTEGER NOT NULL DEFAULT (unixepoch()),
                deleted_at            INTEGER,
                recurring_payment_id  INTEGER,
                UNIQUE(bank_account_id, external_id)
             );
             CREATE INDEX idx_ledger_transaction_date ON ledger_transaction(date);
             CREATE INDEX idx_ledger_transaction_category ON ledger_transaction(category_id);
             CREATE INDEX idx_ledger_transaction_bank_account ON ledger_transaction(bank_account_id);
             CREATE TABLE gocardless_institution_cache (country TEXT);",
        )
        .unwrap();

        conn.execute_batch(
            "INSERT INTO ledger_transaction (amount_pence, description, date, source)
                VALUES (-500, 'Old synced tx', 1700000000, 'sync');
             INSERT INTO ledger_transaction (amount_pence, description, date, source)
                VALUES (1000, 'CSV tx', 1700000001, 'csv_import');
             INSERT INTO ledger_transaction (amount_pence, description, date, source)
                VALUES (-200, 'Manual tx', 1700000002, 'manual');",
        )
        .unwrap();

        // Apply V23 migration SQL directly.
        let v23_sql = include_str!("../../migrations/V23__drop_bank_sync.sql");
        conn.execute_batch(v23_sql).unwrap();

        // 'sync' rows relabelled to 'csv_import_legacy'.
        let sync_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ledger_transaction WHERE source='sync'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sync_count, 0, "no rows should remain with source='sync'");

        let legacy_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ledger_transaction WHERE source='csv_import_legacy'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            legacy_count, 1,
            "one row should be relabelled to 'csv_import_legacy'"
        );

        let csv_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ledger_transaction WHERE source='csv_import'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(csv_count, 1, "'csv_import' row should be untouched");

        let manual_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ledger_transaction WHERE source='manual'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(manual_count, 1, "'manual' row should be untouched");

        // Both bank tables gone.
        for table in ["bank_account", "gocardless_institution_cache"] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 0, "table {table} should be dropped by V23");
        }
    }
}
