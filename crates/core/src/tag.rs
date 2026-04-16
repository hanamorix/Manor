//! Tags + tag_link — universal labels attachable to any entity.

use anyhow::Result;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub created_at: i64,
}

impl Tag {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            color: row.get("color")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Insert a tag if it doesn't exist (case-insensitive on name). Returns the row.
/// Updates color if the tag already exists and `color` differs.
pub fn upsert(conn: &Connection, name: &str, color: &str) -> Result<Tag> {
    conn.execute(
        "INSERT INTO tag (name, color) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET color = excluded.color",
        params![name, color],
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at FROM tag WHERE name = ?1 COLLATE NOCASE",
    )?;
    Ok(stmt.query_row([name], Tag::from_row)?)
}

pub fn list(conn: &Connection) -> Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at FROM tag ORDER BY name ASC",
    )?;
    let tags = stmt.query_map([], Tag::from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tags)
}

pub fn delete_tag(conn: &Connection, id: i64) -> Result<()> {
    // CASCADE cleans tag_link rows.
    conn.execute("DELETE FROM tag WHERE id = ?1", [id])?;
    Ok(())
}

/// Attach a tag to an entity. Idempotent via UNIQUE constraint.
pub fn link(conn: &Connection, tag_id: i64, entity_type: &str, entity_id: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO tag_link (tag_id, entity_type, entity_id)
         VALUES (?1, ?2, ?3)",
        params![tag_id, entity_type, entity_id],
    )?;
    Ok(())
}

pub fn unlink(conn: &Connection, tag_id: i64, entity_type: &str, entity_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM tag_link WHERE tag_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
        params![tag_id, entity_type, entity_id],
    )?;
    Ok(())
}

pub fn tags_for(conn: &Connection, entity_type: &str, entity_id: i64) -> Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.created_at
         FROM tag t
         JOIN tag_link l ON l.tag_id = t.id
         WHERE l.entity_type = ?1 AND l.entity_id = ?2
         ORDER BY t.name ASC",
    )?;
    let tags = stmt.query_map(params![entity_type, entity_id], Tag::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tags)
}

pub fn entities_with_tag(conn: &Connection, tag_id: i64) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT entity_type, entity_id FROM tag_link WHERE tag_id = ?1",
    )?;
    let rows = stmt.query_map([tag_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
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
    fn upsert_creates_tag() {
        let (_d, conn) = fresh_conn();
        let t = upsert(&conn, "urgent", "#f00").unwrap();
        assert_eq!(t.name, "urgent");
        assert_eq!(t.color, "#f00");
    }

    #[test]
    fn upsert_is_case_insensitive_and_updates_color() {
        let (_d, conn) = fresh_conn();
        let a = upsert(&conn, "Urgent", "#f00").unwrap();
        let b = upsert(&conn, "URGENT", "#c00").unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(b.color, "#c00");
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn link_and_tags_for() {
        let (_d, conn) = fresh_conn();
        let t = upsert(&conn, "sample", "#888").unwrap();
        link(&conn, t.id, "event", 42).unwrap();
        let tags = tags_for(&conn, "event", 42).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "sample");
    }

    #[test]
    fn link_is_idempotent() {
        let (_d, conn) = fresh_conn();
        let t = upsert(&conn, "x", "#888").unwrap();
        link(&conn, t.id, "task", 1).unwrap();
        link(&conn, t.id, "task", 1).unwrap();
        assert_eq!(tags_for(&conn, "task", 1).unwrap().len(), 1);
    }

    #[test]
    fn unlink_removes_single_association() {
        let (_d, conn) = fresh_conn();
        let t = upsert(&conn, "x", "#888").unwrap();
        link(&conn, t.id, "task", 1).unwrap();
        link(&conn, t.id, "task", 2).unwrap();
        unlink(&conn, t.id, "task", 1).unwrap();
        assert!(tags_for(&conn, "task", 1).unwrap().is_empty());
        assert_eq!(tags_for(&conn, "task", 2).unwrap().len(), 1);
    }

    #[test]
    fn delete_tag_cascades_links() {
        let (_d, conn) = fresh_conn();
        let t = upsert(&conn, "x", "#888").unwrap();
        link(&conn, t.id, "event", 9).unwrap();
        delete_tag(&conn, t.id).unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM tag_link", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn entities_with_tag_returns_all_rows() {
        let (_d, conn) = fresh_conn();
        let t = upsert(&conn, "x", "#888").unwrap();
        link(&conn, t.id, "task", 1).unwrap();
        link(&conn, t.id, "event", 2).unwrap();
        let mut rows = entities_with_tag(&conn, t.id).unwrap();
        rows.sort();
        assert_eq!(rows, vec![("event".to_string(), 2), ("task".to_string(), 1)]);
    }
}
