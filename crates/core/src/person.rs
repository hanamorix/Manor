//! Person DAL — upgraded for v0.1 completion (kind, contact details, soft-delete).

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

pub const KINDS: &[&str] = &["owner", "member", "contact", "provider", "vendor"];

fn validate_kind(kind: &str) -> Result<()> {
    if KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(anyhow!("invalid kind '{kind}', expected one of {KINDS:?}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Person {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Person {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            kind: row.get("kind")?,
            email: row.get("email")?,
            phone: row.get("phone")?,
            note: row.get("note")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

pub fn insert(
    conn: &Connection,
    name: &str,
    kind: &str,
    email: Option<&str>,
    phone: Option<&str>,
    note: Option<&str>,
) -> Result<Person> {
    validate_kind(kind)?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO person (name, kind, email, phone, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![name, kind, email, phone, note, now],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Person> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, email, phone, note, created_at, updated_at
         FROM person WHERE id = ?1 AND deleted_at IS NULL",
    )?;
    Ok(stmt.query_row([id], Person::from_row)?)
}

pub fn list(conn: &Connection) -> Result<Vec<Person>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, email, phone, note, created_at, updated_at
         FROM person WHERE deleted_at IS NULL ORDER BY name ASC",
    )?;
    let people = stmt.query_map([], Person::from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(people)
}

pub fn list_by_kind(conn: &Connection, kind: &str) -> Result<Vec<Person>> {
    validate_kind(kind)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, email, phone, note, created_at, updated_at
         FROM person WHERE kind = ?1 AND deleted_at IS NULL ORDER BY name ASC",
    )?;
    let people = stmt.query_map([kind], Person::from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(people)
}

pub fn update(
    conn: &Connection,
    id: i64,
    name: &str,
    kind: &str,
    email: Option<&str>,
    phone: Option<&str>,
    note: Option<&str>,
) -> Result<Person> {
    validate_kind(kind)?;
    let now = chrono::Utc::now().timestamp();
    let rows = conn.execute(
        "UPDATE person
         SET name = ?1, kind = ?2, email = ?3, phone = ?4, note = ?5, updated_at = ?6
         WHERE id = ?7 AND deleted_at IS NULL",
        params![name, kind, email, phone, note, now, id],
    )?;
    anyhow::ensure!(rows > 0, "person id={id} not found");
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE person SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Bring back a soft-deleted person (used by Trash restore in Phase B).
pub fn restore(conn: &Connection, id: i64) -> Result<Person> {
    conn.execute(
        "UPDATE person SET deleted_at = NULL, updated_at = unixepoch() WHERE id = ?1",
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
    fn insert_with_all_fields() {
        let (_d, conn) = fresh_conn();
        let p = insert(&conn, "Hana", "owner", Some("h@ex.com"), None, None).unwrap();
        assert_eq!(p.name, "Hana");
        assert_eq!(p.kind, "owner");
        assert_eq!(p.email, Some("h@ex.com".to_string()));
    }

    #[test]
    fn insert_rejects_bad_kind() {
        let (_d, conn) = fresh_conn();
        assert!(insert(&conn, "X", "bogus", None, None, None).is_err());
    }

    #[test]
    fn list_excludes_deleted() {
        let (_d, conn) = fresh_conn();
        let p = insert(&conn, "A", "member", None, None, None).unwrap();
        insert(&conn, "B", "member", None, None, None).unwrap();
        delete(&conn, p.id).unwrap();
        let rows = list(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "B");
    }

    #[test]
    fn list_by_kind_filters() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "H", "owner", None, None, None).unwrap();
        insert(&conn, "K", "member", None, None, None).unwrap();
        insert(&conn, "Plumber", "provider", None, None, None).unwrap();
        assert_eq!(list_by_kind(&conn, "provider").unwrap().len(), 1);
        assert_eq!(list_by_kind(&conn, "member").unwrap().len(), 1);
    }

    #[test]
    fn update_changes_fields_and_bumps_updated_at() {
        let (_d, conn) = fresh_conn();
        let p = insert(&conn, "A", "member", None, None, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let u = update(&conn, p.id, "B", "contact", Some("b@x.com"), None, Some("note")).unwrap();
        assert_eq!(u.name, "B");
        assert_eq!(u.kind, "contact");
        assert!(u.updated_at >= p.updated_at);
    }

    #[test]
    fn restore_resurrects_soft_deleted() {
        let (_d, conn) = fresh_conn();
        let p = insert(&conn, "A", "member", None, None, None).unwrap();
        delete(&conn, p.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        restore(&conn, p.id).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn backfilled_existing_person_rows_default_to_member() {
        // Simulates a pre-V8 person row that got the migration's default 'member' kind.
        let (_d, conn) = fresh_conn();
        conn.execute(
            "INSERT INTO person (id, name, kind, created_at, updated_at) VALUES (?1, ?2, 'member', ?3, ?3)",
            params![99, "OldRow", chrono::Utc::now().timestamp()],
        ).unwrap();
        let got = get(&conn, 99).unwrap();
        assert_eq!(got.kind, "member");
    }
}
