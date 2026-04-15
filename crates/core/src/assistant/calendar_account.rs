//! Calendar account metadata (CalDAV). Password is NOT stored here —
//! it lives in macOS Keychain, keyed by the row id.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarAccount {
    pub id: i64,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    pub last_synced_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
}

impl CalendarAccount {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            display_name: row.get("display_name")?,
            server_url: row.get("server_url")?,
            username: row.get("username")?,
            last_synced_at: row.get("last_synced_at")?,
            last_error: row.get("last_error")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub fn insert(
    conn: &Connection,
    display_name: &str,
    server_url: &str,
    username: &str,
) -> Result<i64> {
    let now_ms = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO calendar_account (display_name, server_url, username, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![display_name, server_url, username, now_ms],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list(conn: &Connection) -> Result<Vec<CalendarAccount>> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, server_url, username, last_synced_at, last_error, created_at
         FROM calendar_account
         ORDER BY created_at",
    )?;
    let rows = stmt
        .query_map([], CalendarAccount::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<CalendarAccount>> {
    let row = conn
        .query_row(
            "SELECT id, display_name, server_url, username, last_synced_at, last_error, created_at
             FROM calendar_account WHERE id = ?1",
            [id],
            CalendarAccount::from_row,
        )
        .optional()?;
    Ok(row)
}

pub fn update_sync_state(
    conn: &Connection,
    id: i64,
    last_synced_at: Option<i64>,
    last_error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE calendar_account SET last_synced_at = ?1, last_error = ?2 WHERE id = ?3",
        params![last_synced_at, last_error, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM calendar_account WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn insert_returns_id() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();
        assert!(id > 0);
    }

    #[test]
    fn list_orders_by_created_at() {
        let (_d, conn) = fresh_conn();
        let a = insert(&conn, "A", "https://a.test", "a").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = insert(&conn, "B", "https://b.test", "b").unwrap();

        let rows = list(&conn).unwrap();
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn update_sync_state_persists_both_timestamp_and_error() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();

        update_sync_state(&conn, id, Some(1_700_000_000), None).unwrap();
        let row = get(&conn, id).unwrap().unwrap();
        assert_eq!(row.last_synced_at, Some(1_700_000_000));
        assert_eq!(row.last_error, None);

        update_sync_state(&conn, id, None, Some("bad credentials")).unwrap();
        let row = get(&conn, id).unwrap().unwrap();
        assert_eq!(row.last_synced_at, None);
        assert_eq!(row.last_error.as_deref(), Some("bad credentials"));
    }

    #[test]
    fn delete_cascades_events() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c").unwrap();
        // Insert an event directly (event DAL arrives in Task 3 — inline SQL here).
        conn.execute(
            "INSERT INTO event (calendar_account_id, external_id, title, start_at, end_at, created_at)
             VALUES (?1, 'uid-1', 'Test', 1, 2, 3)",
            [id],
        )
        .unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM event", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);

        delete(&conn, id).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM event", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "CASCADE should have wiped the event");
    }
}
