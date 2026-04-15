//! Tasks — the user's open / completed to-dos.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub due_date: Option<String>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub proposal_id: Option<i64>,
}

impl Task {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            due_date: row.get("due_date")?,
            completed_at: row.get("completed_at")?,
            created_at: row.get("created_at")?,
            proposal_id: row.get("proposal_id")?,
        })
    }
}

/// Insert a new task. Returns the new row id.
pub fn insert(
    conn: &Connection,
    title: &str,
    due_date: Option<&str>,
    proposal_id: Option<i64>,
) -> Result<i64> {
    let now_ms = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO task (title, due_date, completed_at, created_at, proposal_id)
         VALUES (?1, ?2, NULL, ?3, ?4)",
        params![title, due_date, now_ms, proposal_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// All open tasks (completed_at IS NULL), ordered with NULL due_dates last.
/// Used by Phase 3c's prompt context — it wants every open task Manor should
/// know about, regardless of due date.
pub fn list_open(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, due_date, completed_at, created_at, proposal_id
         FROM task
         WHERE completed_at IS NULL
         ORDER BY (due_date IS NULL), due_date, created_at",
    )?;
    let rows = stmt
        .query_map([], Task::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Open tasks due today or with no due date. Used by Phase 3a's Today view
/// so tasks scheduled for future days don't appear in today's list.
pub fn list_today_open(conn: &Connection, today_iso: &str) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, due_date, completed_at, created_at, proposal_id
         FROM task
         WHERE completed_at IS NULL AND (due_date IS NULL OR due_date = ?1)
         ORDER BY (due_date IS NULL), due_date, created_at",
    )?;
    let rows = stmt
        .query_map([today_iso], Task::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Mark a task complete (set completed_at to now).
pub fn complete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE task SET completed_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Undo completion (set completed_at to NULL). Called inside the 4s undo window.
pub fn undo_complete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("UPDATE task SET completed_at = NULL WHERE id = ?1", [id])?;
    Ok(())
}

/// Rename a task.
pub fn update_title(conn: &Connection, id: i64, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE task SET title = ?1 WHERE id = ?2",
        params![title, id],
    )?;
    Ok(())
}

/// Hard-delete a task.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM task WHERE id = ?1", [id])?;
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
    fn insert_returns_id_and_persists() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Pick up prescription", Some("2026-04-15"), None).unwrap();
        assert!(id > 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn list_open_excludes_completed() {
        let (_d, conn) = fresh_conn();
        let a = insert(&conn, "A", Some("2026-04-15"), None).unwrap();
        let _b = insert(&conn, "B", Some("2026-04-15"), None).unwrap();
        complete(&conn, a).unwrap();

        let open = list_open(&conn).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].title, "B");
    }

    #[test]
    fn list_open_orders_by_due_date_then_created_at() {
        let (_d, conn) = fresh_conn();
        // No due date — should sort to the end.
        insert(&conn, "no_due", None, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        // Far due date.
        insert(&conn, "later", Some("2026-04-30"), None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        // Earlier due date.
        insert(&conn, "earlier", Some("2026-04-15"), None).unwrap();

        let open = list_open(&conn).unwrap();
        let titles: Vec<&str> = open.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["earlier", "later", "no_due"]);
    }

    #[test]
    fn list_today_open_filters_by_today_and_includes_no_due() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "today_task", Some("2026-04-15"), None).unwrap();
        insert(&conn, "future_task", Some("2026-04-30"), None).unwrap();
        insert(&conn, "no_due_task", None, None).unwrap();

        let today = list_today_open(&conn, "2026-04-15").unwrap();
        let titles: Vec<&str> = today.iter().map(|t| t.title.as_str()).collect();
        // future_task excluded; today_task and no_due_task included.
        assert!(titles.contains(&"today_task"));
        assert!(titles.contains(&"no_due_task"));
        assert!(!titles.contains(&"future_task"));
    }

    #[test]
    fn complete_then_undo_round_trip() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "T", Some("2026-04-15"), None).unwrap();
        complete(&conn, id).unwrap();
        assert_eq!(list_open(&conn).unwrap().len(), 0);

        undo_complete(&conn, id).unwrap();
        let open = list_open(&conn).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, id);
        assert!(open[0].completed_at.is_none());
    }

    #[test]
    fn update_title_persists() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Old", Some("2026-04-15"), None).unwrap();
        update_title(&conn, id, "New").unwrap();
        let open = list_open(&conn).unwrap();
        assert_eq!(open[0].title, "New");
    }

    #[test]
    fn delete_removes_row() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Doomed", Some("2026-04-15"), None).unwrap();
        delete(&conn, id).unwrap();
        assert_eq!(list_open(&conn).unwrap().len(), 0);
    }
}
