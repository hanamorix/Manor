//! Category DAL — CRUD for transaction categories.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub emoji: String,
    pub is_income: bool,
    pub sort_order: i32,
    pub is_default: bool,
    pub deleted_at: Option<i64>,
}

impl Category {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            emoji: row.get("emoji")?,
            is_income: row.get::<_, i64>("is_income")? != 0,
            sort_order: row.get("sort_order")?,
            is_default: row.get::<_, i64>("is_default")? != 0,
            deleted_at: row.get("deleted_at")?,
        })
    }
}

pub fn list(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, emoji, is_income, sort_order, is_default, deleted_at
         FROM category
         WHERE deleted_at IS NULL
         ORDER BY sort_order, id",
    )?;
    let rows = stmt
        .query_map([], Category::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn insert(
    conn: &Connection,
    name: &str,
    emoji: &str,
    is_income: bool,
) -> Result<Category> {
    let max_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) FROM category WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO category (name, emoji, is_income, sort_order, is_default)
         VALUES (?1, ?2, ?3, ?4, 0)",
        params![name, emoji, is_income as i64, max_order + 1],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, name: &str, emoji: &str) -> Result<Category> {
    let rows = conn.execute(
        "UPDATE category SET name = ?1, emoji = ?2 WHERE id = ?3",
        params![name, emoji, id],
    )?;
    anyhow::ensure!(rows > 0, "category id={} not found", id);
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let is_default: i64 = conn.query_row(
        "SELECT is_default FROM category WHERE id = ?1",
        [id],
        |r| r.get(0),
    )?;
    anyhow::ensure!(is_default == 0, "cannot delete a default category");
    // Reassign orphaned transactions to Other (id = 9)
    conn.execute(
        "UPDATE ledger_transaction SET category_id = 9
         WHERE category_id = ?1 AND deleted_at IS NULL",
        [id],
    )?;
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE category SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

fn get(conn: &Connection, id: i64) -> Result<Category> {
    let mut stmt = conn.prepare(
        "SELECT id, name, emoji, is_income, sort_order, is_default, deleted_at
         FROM category WHERE id = ?1",
    )?;
    Ok(stmt.query_row([id], Category::from_row)?)
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
    fn seeds_ten_default_categories() {
        let (_d, conn) = fresh_conn();
        let cats = list(&conn).unwrap();
        assert_eq!(cats.len(), 10, "expected 10 seeded categories");
        assert!(cats.iter().any(|c| c.name == "Groceries"));
        assert!(cats.iter().any(|c| c.name == "Income" && c.is_income));
    }

    #[test]
    fn insert_custom_category_appended_at_end() {
        let (_d, conn) = fresh_conn();
        let cat = insert(&conn, "Pets", "🐶", false).unwrap();
        assert_eq!(cat.name, "Pets");
        assert!(!cat.is_default);
        let all = list(&conn).unwrap();
        assert_eq!(all.last().unwrap().name, "Pets");
    }

    #[test]
    fn update_renames_category() {
        let (_d, conn) = fresh_conn();
        let cat = insert(&conn, "Pets", "🐶", false).unwrap();
        let updated = update(&conn, cat.id, "Animals", "🐾").unwrap();
        assert_eq!(updated.name, "Animals");
        assert_eq!(updated.emoji, "🐾");
    }

    #[test]
    fn cannot_delete_default_category() {
        let (_d, conn) = fresh_conn();
        let err = delete(&conn, 1).unwrap_err(); // id=1 is Groceries (default)
        assert!(err.to_string().contains("cannot delete a default category"));
    }

    #[test]
    fn delete_custom_category_soft_deletes_it() {
        let (_d, conn) = fresh_conn();
        let cat = insert(&conn, "Hobbies", "🎸", false).unwrap();
        delete(&conn, cat.id).unwrap();
        let all = list(&conn).unwrap();
        assert!(!all.iter().any(|c| c.id == cat.id));
    }
}
