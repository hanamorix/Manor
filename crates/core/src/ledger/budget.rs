//! Budget DAL — monthly limits per category + spend rollup.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::ledger::transaction::{month_end_ts, month_start_ts};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Budget {
    pub id: i64,
    pub category_id: i64,
    pub amount_pence: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySpend {
    pub category_id: i64,
    pub category_name: String,
    pub category_emoji: String,
    pub spent_pence: i64,
    pub budget_pence: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySummary {
    pub total_in_pence: i64,
    pub total_out_pence: i64,
    pub by_category: Vec<CategorySpend>,
}

impl Budget {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            category_id: row.get("category_id")?,
            amount_pence: row.get("amount_pence")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub fn list(conn: &Connection) -> Result<Vec<Budget>> {
    let mut stmt = conn.prepare(
        "SELECT id, category_id, amount_pence, created_at
         FROM budget WHERE deleted_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], Budget::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Insert or update the monthly budget for a category.
/// If the category had a soft-deleted budget, it is restored.
pub fn upsert(conn: &Connection, category_id: i64, amount_pence: i64) -> Result<Budget> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO budget (category_id, amount_pence, created_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(category_id) DO UPDATE
           SET amount_pence = excluded.amount_pence,
               deleted_at   = NULL",
        params![category_id, amount_pence, now],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM budget WHERE category_id = ?1",
        [category_id],
        |r| r.get(0),
    )?;
    Ok(conn.query_row(
        "SELECT id, category_id, amount_pence, created_at FROM budget WHERE id = ?1",
        [id],
        Budget::from_row,
    )?)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE budget SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Aggregate total in/out and per-category spend vs budget for a month.
pub fn monthly_summary(conn: &Connection, year: i32, month: u32) -> Result<MonthlySummary> {
    let start = month_start_ts(year, month);
    let end = month_end_ts(year, month);

    let (total_in, total_out): (i64, i64) = conn.query_row(
        "SELECT
           COALESCE(SUM(CASE WHEN amount_pence > 0 THEN amount_pence ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN amount_pence < 0 THEN -amount_pence ELSE 0 END), 0)
         FROM ledger_transaction
         WHERE deleted_at IS NULL AND date >= ?1 AND date < ?2",
        params![start, end],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.emoji,
                COALESCE(SUM(-t.amount_pence), 0) AS spent,
                b.amount_pence AS budget_pence
         FROM category c
         LEFT JOIN ledger_transaction t
           ON t.category_id = c.id
          AND t.deleted_at IS NULL
          AND t.date >= ?1 AND t.date < ?2
          AND t.amount_pence < 0
         LEFT JOIN budget b ON b.category_id = c.id AND b.deleted_at IS NULL
         WHERE c.deleted_at IS NULL AND c.is_income = 0
         GROUP BY c.id
         ORDER BY spent DESC",
    )?;
    let by_category = stmt
        .query_map(params![start, end], |row| {
            Ok(CategorySpend {
                category_id: row.get(0)?,
                category_name: row.get(1)?,
                category_emoji: row.get(2)?,
                spent_pence: row.get(3)?,
                budget_pence: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(MonthlySummary {
        total_in_pence: total_in,
        total_out_pence: total_out,
        by_category,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::transaction;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn april_ts(day: u32) -> i64 {
        use chrono::{TimeZone, Utc};
        Utc.with_ymd_and_hms(2026, 4, day, 12, 0, 0).unwrap().timestamp()
    }

    #[test]
    fn upsert_creates_budget() {
        let (_d, conn) = fresh_conn();
        let b = upsert(&conn, 1, 40000).unwrap(); // £400 for Groceries
        assert_eq!(b.category_id, 1);
        assert_eq!(b.amount_pence, 40000);
    }

    #[test]
    fn upsert_updates_existing_budget() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, 1, 40000).unwrap();
        let updated = upsert(&conn, 1, 50000).unwrap();
        assert_eq!(updated.amount_pence, 50000);
        assert_eq!(list(&conn).unwrap().len(), 1); // still only one row
    }

    #[test]
    fn delete_removes_budget_from_list() {
        let (_d, conn) = fresh_conn();
        let b = upsert(&conn, 1, 40000).unwrap();
        delete(&conn, b.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }

    #[test]
    fn upsert_restores_soft_deleted_budget() {
        let (_d, conn) = fresh_conn();
        let b = upsert(&conn, 1, 40000).unwrap();
        delete(&conn, b.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        let restored = upsert(&conn, 1, 60000).unwrap();
        assert_eq!(restored.amount_pence, 60000);
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn monthly_summary_totals_in_and_out() {
        let (_d, conn) = fresh_conn();
        transaction::insert(&conn, 320000, "GBP", "Salary", None, Some(10), april_ts(1), None).unwrap();
        transaction::insert(&conn, -3420, "GBP", "Tesco", None, Some(1), april_ts(5), None).unwrap();
        transaction::insert(&conn, -1850, "GBP", "Deliveroo", None, Some(2), april_ts(10), None).unwrap();

        let s = monthly_summary(&conn, 2026, 4).unwrap();
        assert_eq!(s.total_in_pence, 320000);
        assert_eq!(s.total_out_pence, 5270);
    }

    #[test]
    fn monthly_summary_category_spend_and_budget() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, 1, 40000).unwrap(); // £400 grocery budget
        transaction::insert(&conn, -3420, "GBP", "Tesco", None, Some(1), april_ts(5), None).unwrap();

        let s = monthly_summary(&conn, 2026, 4).unwrap();
        let groceries = s.by_category.iter().find(|c| c.category_id == 1).unwrap();
        assert_eq!(groceries.spent_pence, 3420);
        assert_eq!(groceries.budget_pence, Some(40000));

        let eating_out = s.by_category.iter().find(|c| c.category_id == 2).unwrap();
        assert_eq!(eating_out.spent_pence, 0);
        assert_eq!(eating_out.budget_pence, None);
    }
}
