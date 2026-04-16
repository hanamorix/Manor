//! Note DAL — markdown notes optionally attached to another entity.

use anyhow::Result;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    pub body_md: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Note {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            body_md: row.get("body_md")?,
            entity_type: row.get("entity_type")?,
            entity_id: row.get("entity_id")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

pub fn insert(
    conn: &Connection,
    body_md: &str,
    entity_type: Option<&str>,
    entity_id: Option<i64>,
) -> Result<Note> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO note (body_md, entity_type, entity_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![body_md, entity_type, entity_id, now],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Note> {
    let mut stmt = conn.prepare(
        "SELECT id, body_md, entity_type, entity_id, created_at, updated_at
         FROM note WHERE id = ?1 AND deleted_at IS NULL",
    )?;
    Ok(stmt.query_row([id], Note::from_row)?)
}

pub fn list_for(conn: &Connection, entity_type: &str, entity_id: i64) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare(
        "SELECT id, body_md, entity_type, entity_id, created_at, updated_at
         FROM note
         WHERE entity_type = ?1 AND entity_id = ?2 AND deleted_at IS NULL
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![entity_type, entity_id], Note::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_orphans(conn: &Connection) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare(
        "SELECT id, body_md, entity_type, entity_id, created_at, updated_at
         FROM note
         WHERE entity_type IS NULL AND deleted_at IS NULL
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], Note::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn update(conn: &Connection, id: i64, body_md: &str) -> Result<Note> {
    let now = chrono::Utc::now().timestamp();
    let rows = conn.execute(
        "UPDATE note SET body_md = ?1, updated_at = ?2
         WHERE id = ?3 AND deleted_at IS NULL",
        params![body_md, now, id],
    )?;
    anyhow::ensure!(rows > 0, "note id={id} not found");
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE note SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn restore(conn: &Connection, id: i64) -> Result<Note> {
    conn.execute(
        "UPDATE note SET deleted_at = NULL, updated_at = unixepoch() WHERE id = ?1",
        [id],
    )?;
    get(conn, id)
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
    fn insert_attached_note() {
        let (_d, conn) = fresh_conn();
        let n = insert(
            &conn,
            "Boiler last serviced 2024",
            Some("attachment"),
            Some(7),
        )
        .unwrap();
        assert_eq!(n.entity_type.as_deref(), Some("attachment"));
        assert_eq!(n.entity_id, Some(7));
    }

    #[test]
    fn insert_orphan_note() {
        let (_d, conn) = fresh_conn();
        let n = insert(&conn, "Random thought", None, None).unwrap();
        assert!(n.entity_type.is_none());
    }

    #[test]
    fn list_for_returns_entity_notes() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "n1", Some("event"), Some(1)).unwrap();
        insert(&conn, "n2", Some("event"), Some(1)).unwrap();
        insert(&conn, "other", Some("event"), Some(2)).unwrap();
        let rows = list_for(&conn, "event", 1).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn list_orphans_filters() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "orphan", None, None).unwrap();
        insert(&conn, "attached", Some("task"), Some(1)).unwrap();
        let rows = list_orphans(&conn).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn update_edits_body() {
        let (_d, conn) = fresh_conn();
        let n = insert(&conn, "a", None, None).unwrap();
        let u = update(&conn, n.id, "b").unwrap();
        assert_eq!(u.body_md, "b");
    }

    #[test]
    fn delete_hides_from_list_and_restore_brings_back() {
        let (_d, conn) = fresh_conn();
        let n = insert(&conn, "x", Some("task"), Some(5)).unwrap();
        delete(&conn, n.id).unwrap();
        assert!(list_for(&conn, "task", 5).unwrap().is_empty());
        restore(&conn, n.id).unwrap();
        assert_eq!(list_for(&conn, "task", 5).unwrap().len(), 1);
    }
}
