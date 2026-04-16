//! Persisted calendar list — one row per calendar URL per account.

use anyhow::Result;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Calendar {
    pub id: i64,
    pub calendar_account_id: i64,
    pub url: String,
    pub display_name: Option<String>,
}

impl Calendar {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            calendar_account_id: row.get("calendar_account_id")?,
            url: row.get("url")?,
            display_name: row.get("display_name")?,
        })
    }
}

/// Upsert a calendar URL. INSERT OR IGNORE — never overwrites existing rows.
pub fn upsert(
    conn: &Connection,
    account_id: i64,
    url: &str,
    display_name: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO calendar (calendar_account_id, url, display_name)
         VALUES (?1, ?2, ?3)",
        params![account_id, url, display_name],
    )?;
    Ok(())
}

pub fn list(conn: &Connection, account_id: i64) -> Result<Vec<Calendar>> {
    let mut stmt = conn.prepare(
        "SELECT id, calendar_account_id, url, display_name
         FROM calendar WHERE calendar_account_id = ?1
         ORDER BY id",
    )?;
    let rows = stmt
        .query_map([account_id], Calendar::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::{calendar_account, db};
    use tempfile::tempdir;

    fn fresh(account: &str) -> (tempfile::TempDir, Connection, i64) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let id = calendar_account::insert(&conn, account, "https://cal.test", "u").unwrap();
        (dir, conn, id)
    }

    #[test]
    fn upsert_then_list() {
        let (_d, conn, aid) = fresh("A");
        upsert(&conn, aid, "https://cal.test/home/work/", Some("Work")).unwrap();
        upsert(
            &conn,
            aid,
            "https://cal.test/home/personal/",
            Some("Personal"),
        )
        .unwrap();
        let cals = list(&conn, aid).unwrap();
        assert_eq!(cals.len(), 2);
        assert_eq!(cals[0].display_name.as_deref(), Some("Work"));
    }

    #[test]
    fn upsert_is_idempotent() {
        let (_d, conn, aid) = fresh("A");
        upsert(&conn, aid, "https://cal.test/home/work/", Some("Work")).unwrap();
        upsert(&conn, aid, "https://cal.test/home/work/", Some("Work")).unwrap();
        let cals = list(&conn, aid).unwrap();
        assert_eq!(cals.len(), 1);
    }

    #[test]
    fn list_scoped_to_account() {
        let (_d, conn, aid) = fresh("A");
        let bid = calendar_account::insert(&conn, "B", "https://b.test", "u").unwrap();
        upsert(&conn, aid, "https://cal.test/home/work/", None).unwrap();
        upsert(&conn, bid, "https://b.test/home/cal/", None).unwrap();
        assert_eq!(list(&conn, aid).unwrap().len(), 1);
        assert_eq!(list(&conn, bid).unwrap().len(), 1);
    }

    #[test]
    fn cascade_delete_removes_calendars() {
        let (_d, conn, aid) = fresh("A");
        upsert(&conn, aid, "https://cal.test/home/work/", None).unwrap();
        calendar_account::delete(&conn, aid).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM calendar", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
