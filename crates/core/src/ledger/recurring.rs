//! RecurringPayment DAL — templates that auto-insert transactions monthly.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecurringPayment {
    pub id: i64,
    pub description: String,
    pub amount_pence: i64,
    pub currency: String,
    pub category_id: Option<i64>,
    pub day_of_month: i64,
    pub active: bool,
    pub note: Option<String>,
    pub created_at: i64,
}

impl RecurringPayment {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let active: i64 = row.get("active")?;
        Ok(Self {
            id: row.get("id")?,
            description: row.get("description")?,
            amount_pence: row.get("amount_pence")?,
            currency: row.get("currency")?,
            category_id: row.get("category_id")?,
            day_of_month: row.get("day_of_month")?,
            active: active != 0,
            note: row.get("note")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub fn insert(
    conn: &Connection,
    description: &str,
    amount_pence: i64,
    currency: &str,
    category_id: Option<i64>,
    day_of_month: i64,
    note: Option<&str>,
) -> Result<RecurringPayment> {
    anyhow::ensure!(
        (1..=28).contains(&day_of_month),
        "day_of_month must be 1..=28, got {day_of_month}"
    );
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO recurring_payment
         (description, amount_pence, currency, category_id, day_of_month, active, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
        params![
            description,
            amount_pence,
            currency,
            category_id,
            day_of_month,
            note,
            now
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn list(conn: &Connection) -> Result<Vec<RecurringPayment>> {
    let mut stmt = conn.prepare(
        "SELECT id, description, amount_pence, currency, category_id, day_of_month,
                active, note, created_at
         FROM recurring_payment
         WHERE deleted_at IS NULL
         ORDER BY day_of_month ASC, id ASC",
    )?;
    let results = stmt
        .query_map([], RecurringPayment::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    conn: &Connection,
    id: i64,
    description: &str,
    amount_pence: i64,
    category_id: Option<i64>,
    day_of_month: i64,
    active: bool,
    note: Option<&str>,
) -> Result<RecurringPayment> {
    anyhow::ensure!(
        (1..=28).contains(&day_of_month),
        "day_of_month must be 1..=28, got {day_of_month}"
    );
    let rows = conn.execute(
        "UPDATE recurring_payment
         SET description = ?1, amount_pence = ?2, category_id = ?3,
             day_of_month = ?4, active = ?5, note = ?6
         WHERE id = ?7 AND deleted_at IS NULL",
        params![
            description,
            amount_pence,
            category_id,
            day_of_month,
            active as i64,
            note,
            id
        ],
    )?;
    anyhow::ensure!(rows > 0, "recurring_payment id={id} not found");
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE recurring_payment SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

fn get(conn: &Connection, id: i64) -> Result<RecurringPayment> {
    let mut stmt = conn.prepare(
        "SELECT id, description, amount_pence, currency, category_id, day_of_month,
                active, note, created_at
         FROM recurring_payment WHERE id = ?1",
    )?;
    Ok(stmt.query_row([id], RecurringPayment::from_row)?)
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
    fn insert_creates_row() {
        let (_d, conn) = fresh_conn();
        let r = insert(&conn, "Netflix", 1299, "GBP", Some(5), 15, None).unwrap();
        assert_eq!(r.description, "Netflix");
        assert_eq!(r.amount_pence, 1299);
        assert_eq!(r.day_of_month, 15);
        assert!(r.active);
    }

    #[test]
    fn insert_rejects_invalid_day_of_month() {
        let (_d, conn) = fresh_conn();
        assert!(insert(&conn, "Bad", 100, "GBP", None, 29, None).is_err());
        assert!(insert(&conn, "Bad", 100, "GBP", None, 0, None).is_err());
    }

    #[test]
    fn list_orders_by_day_of_month() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        insert(&conn, "Netflix", 1299, "GBP", None, 15, None).unwrap();
        insert(&conn, "Gym", 3000, "GBP", None, 7, None).unwrap();
        let rows = list(&conn).unwrap();
        let days: Vec<_> = rows.iter().map(|r| r.day_of_month).collect();
        assert_eq!(days, vec![1, 7, 15]);
    }

    #[test]
    fn update_changes_fields() {
        let (_d, conn) = fresh_conn();
        let r = insert(&conn, "Old", 500, "GBP", None, 10, None).unwrap();
        let u = update(&conn, r.id, "New", 800, Some(2), 20, false, Some("paused")).unwrap();
        assert_eq!(u.description, "New");
        assert_eq!(u.amount_pence, 800);
        assert!(!u.active);
        assert_eq!(u.note, Some("paused".to_string()));
    }

    #[test]
    fn delete_soft_deletes() {
        let (_d, conn) = fresh_conn();
        let r = insert(&conn, "Gone", 100, "GBP", None, 1, None).unwrap();
        delete(&conn, r.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
