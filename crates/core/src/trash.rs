//! Trash aggregator — unions soft-deleted rows across every table that has a
//! `deleted_at` column. Restore + permanent-delete route to each table's DAL.

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// One row in the Trash view. `entity_type` is the table name; `entity_id` is
/// the primary key. `title` is a human-readable label extracted per-table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashEntry {
    pub entity_type: String,
    pub entity_id: i64,
    pub title: String,
    pub deleted_at: i64,
}

/// Known soft-deletable tables + their title column SQL expression.
/// Adding a new table here is how Trash picks it up.
const REGISTRY: &[(&str, &str)] = &[
    ("person", "name"),
    ("note", "substr(body_md, 1, 60)"),
    ("attachment", "original_name"),
    ("task", "title"),
    ("event", "title"),
    ("chore", "title"),
    ("time_block", "title"),
    ("calendar_account", "email"),
    ("ledger_transaction", "description"),
    ("budget", "CAST(category_id AS TEXT)"),
    ("category", "name"),
    ("bank_account", "account_name"),
];

pub fn list_all(conn: &Connection) -> Result<Vec<TrashEntry>> {
    let mut out = Vec::new();
    for (table, title_expr) in REGISTRY {
        let sql = format!(
            "SELECT id, {title_expr} AS title, deleted_at
             FROM {table} WHERE deleted_at IS NOT NULL
             ORDER BY deleted_at DESC"
        );
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            // Table doesn't have deleted_at or doesn't exist in this schema version.
            Err(_) => continue,
        };
        let rows = stmt.query_map([], |row| {
            Ok(TrashEntry {
                entity_type: table.to_string(),
                entity_id: row.get(0)?,
                title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                deleted_at: row.get(2)?,
            })
        })?;
        for r in rows {
            out.push(r?);
        }
    }
    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(out)
}

pub fn restore(conn: &Connection, entity_type: &str, entity_id: i64) -> Result<()> {
    validate_table(entity_type)?;
    let sql = format!("UPDATE {entity_type} SET deleted_at = NULL WHERE id = ?1");
    let rows = conn.execute(&sql, params![entity_id])?;
    anyhow::ensure!(rows > 0, "{entity_type} id={entity_id} not found");
    Ok(())
}

pub fn permanent_delete(conn: &Connection, entity_type: &str, entity_id: i64) -> Result<()> {
    validate_table(entity_type)?;
    let sql = format!("DELETE FROM {entity_type} WHERE id = ?1 AND deleted_at IS NOT NULL");
    conn.execute(&sql, params![entity_id])?;
    Ok(())
}

/// Hard-delete every row across every table whose `deleted_at` is older than `cutoff_ts`.
/// Returns the total number of rows removed per-table for logging.
pub fn empty_older_than(conn: &Connection, cutoff_ts: i64) -> Result<Vec<(String, usize)>> {
    let mut totals = Vec::new();
    for (table, _title_expr) in REGISTRY {
        let sql = format!(
            "DELETE FROM {table}
             WHERE deleted_at IS NOT NULL AND deleted_at < ?1"
        );
        let n = match conn.execute(&sql, params![cutoff_ts]) {
            Ok(n) => n,
            Err(_) => continue, // table may not have deleted_at
        };
        if n > 0 {
            totals.push((table.to_string(), n));
        }
    }
    Ok(totals)
}

/// Hard-delete ALL soft-deleted rows (used by "Empty Trash Now" button).
pub fn empty_all(conn: &Connection) -> Result<Vec<(String, usize)>> {
    let mut totals = Vec::new();
    for (table, _title_expr) in REGISTRY {
        let sql = format!("DELETE FROM {table} WHERE deleted_at IS NOT NULL");
        let n = match conn.execute(&sql, []) {
            Ok(n) => n,
            Err(_) => continue,
        };
        if n > 0 {
            totals.push((table.to_string(), n));
        }
    }
    Ok(totals)
}

fn validate_table(entity_type: &str) -> Result<()> {
    if REGISTRY.iter().any(|(t, _)| *t == entity_type) {
        Ok(())
    } else {
        Err(anyhow!("unknown table '{entity_type}' for trash operation"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use chrono::Utc;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn list_all_empty_when_nothing_deleted() {
        let (_d, conn) = fresh_conn();
        assert!(list_all(&conn).unwrap().is_empty());
    }

    #[test]
    fn list_all_surfaces_soft_deleted_person() {
        let (_d, conn) = fresh_conn();
        let p = crate::person::insert(&conn, "Alice", "member", None, None, None).unwrap();
        crate::person::delete(&conn, p.id).unwrap();
        let rows = list_all(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_type, "person");
        assert_eq!(rows[0].title, "Alice");
    }

    #[test]
    fn list_all_surfaces_soft_deleted_note() {
        let (_d, conn) = fresh_conn();
        let n = crate::note::insert(&conn, "a long body text here and there", None, None).unwrap();
        crate::note::delete(&conn, n.id).unwrap();
        let rows = list_all(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_type, "note");
        assert!(rows[0].title.starts_with("a long"));
    }

    #[test]
    fn list_all_orders_by_deleted_at_desc() {
        let (_d, conn) = fresh_conn();
        let a = crate::person::insert(&conn, "A", "member", None, None, None).unwrap();
        let b = crate::person::insert(&conn, "B", "member", None, None, None).unwrap();
        // Manual deleted_at to control order.
        conn.execute("UPDATE person SET deleted_at = 100 WHERE id = ?1", [a.id])
            .unwrap();
        conn.execute("UPDATE person SET deleted_at = 200 WHERE id = ?1", [b.id])
            .unwrap();
        let rows = list_all(&conn).unwrap();
        assert_eq!(rows[0].title, "B"); // newer deletion first
        assert_eq!(rows[1].title, "A");
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn) = fresh_conn();
        let p = crate::person::insert(&conn, "A", "member", None, None, None).unwrap();
        crate::person::delete(&conn, p.id).unwrap();
        restore(&conn, "person", p.id).unwrap();
        assert!(list_all(&conn).unwrap().is_empty());
        assert!(crate::person::get(&conn, p.id).is_ok());
    }

    #[test]
    fn restore_rejects_unknown_table() {
        let (_d, conn) = fresh_conn();
        assert!(restore(&conn, "nope", 1).is_err());
    }

    #[test]
    fn permanent_delete_removes_row_completely() {
        let (_d, conn) = fresh_conn();
        let p = crate::person::insert(&conn, "A", "member", None, None, None).unwrap();
        crate::person::delete(&conn, p.id).unwrap();
        permanent_delete(&conn, "person", p.id).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM person WHERE id = ?1", [p.id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn empty_older_than_respects_cutoff() {
        let (_d, conn) = fresh_conn();
        let old = crate::person::insert(&conn, "Old", "member", None, None, None).unwrap();
        let fresh = crate::person::insert(&conn, "Fresh", "member", None, None, None).unwrap();
        conn.execute("UPDATE person SET deleted_at = 100 WHERE id = ?1", [old.id])
            .unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE person SET deleted_at = ?1 WHERE id = ?2",
            params![now, fresh.id],
        )
        .unwrap();

        let cutoff = now - 86400; // 1 day ago
        let totals = empty_older_than(&conn, cutoff).unwrap();
        assert!(totals.iter().any(|(t, n)| t == "person" && *n == 1));

        // old row is gone; fresh remains (still soft-deleted)
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM person WHERE id = ?1",
                [fresh.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn empty_all_nukes_all_soft_deleted() {
        let (_d, conn) = fresh_conn();
        let p = crate::person::insert(&conn, "A", "member", None, None, None).unwrap();
        crate::person::delete(&conn, p.id).unwrap();
        let n = crate::note::insert(&conn, "note body", None, None).unwrap();
        crate::note::delete(&conn, n.id).unwrap();
        empty_all(&conn).unwrap();
        assert!(list_all(&conn).unwrap().is_empty());
    }
}
