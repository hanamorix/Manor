//! RecurringPayment DAL — templates that auto-insert transactions monthly.

use anyhow::Result;
use chrono::{DateTime, Datelike, TimeZone, Utc};
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

/// Run auto-insert for all active recurring payments. For each payment whose
/// `day_of_month <= today.day` and which has no transaction in the current
/// calendar month yet, insert one. All-or-nothing transaction.
///
/// Returns the number of transactions inserted.
pub fn auto_insert_due(conn: &mut Connection, now: DateTime<Utc>) -> Result<usize> {
    let tx = conn.transaction()?;
    let year = now.year();
    let month = now.month();
    let today_dom = now.day() as i64;
    let month_start = crate::ledger::transaction::month_start_ts(year, month);
    let month_end = crate::ledger::transaction::month_end_ts(year, month);
    let today_midnight = Utc
        .with_ymd_and_hms(year, month, now.day(), 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid ymd"))?
        .timestamp();

    let due: Vec<(i64, String, i64, String, Option<i64>)> = {
        let mut stmt = tx.prepare(
            "SELECT r.id, r.description, r.amount_pence, r.currency, r.category_id
             FROM recurring_payment r
             WHERE r.deleted_at IS NULL
               AND r.active = 1
               AND r.day_of_month <= ?1
               AND NOT EXISTS (
                   SELECT 1 FROM ledger_transaction t
                   WHERE t.recurring_payment_id = r.id
                     AND t.deleted_at IS NULL
                     AND t.date >= ?2 AND t.date < ?3
               )",
        )?;
        let rows = stmt
            .query_map(params![today_dom, month_start, month_end], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };

    let inserted = due.len();
    for (id, description, amount_pence, currency, category_id) in due {
        // Debit: store as negative pence.
        let signed = -amount_pence.abs();
        tx.execute(
            "INSERT INTO ledger_transaction
             (bank_account_id, amount_pence, currency, description, merchant,
              category_id, date, source, note, recurring_payment_id, created_at)
             VALUES (NULL, ?1, ?2, ?3, NULL, ?4, ?5, 'recurring', NULL, ?6, ?7)",
            params![
                signed,
                currency,
                description,
                category_id,
                today_midnight,
                id,
                Utc::now().timestamp()
            ],
        )?;
    }
    tx.commit()?;
    Ok(inserted)
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

    use crate::ledger::transaction;

    fn utc_day(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap()
    }

    #[test]
    fn auto_insert_inserts_when_day_reached_and_no_existing() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        assert_eq!(count, 1);
        let txns = transaction::list_by_month(&conn, 2026, 4).unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].source, "recurring");
        assert_eq!(txns[0].amount_pence, -100000);
        assert!(txns[0].recurring_payment_id.is_some());
    }

    #[test]
    fn auto_insert_skips_when_day_not_reached() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 20, None).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 10)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn auto_insert_is_idempotent_within_month() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 20)).unwrap();
        assert_eq!(count, 0);
        let txns = transaction::list_by_month(&conn, 2026, 4).unwrap();
        assert_eq!(txns.len(), 1);
    }

    #[test]
    fn auto_insert_re_runs_next_month() {
        let (_d, mut conn) = fresh_conn();
        insert(&conn, "Rent", 100000, "GBP", None, 1, None).unwrap();
        auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 5, 2)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn auto_insert_skips_inactive() {
        let (_d, mut conn) = fresh_conn();
        let r = insert(&conn, "Paused", 500, "GBP", None, 1, None).unwrap();
        update(&conn, r.id, "Paused", 500, None, 1, false, None).unwrap();
        let count = auto_insert_due(&mut conn, utc_day(2026, 4, 16)).unwrap();
        assert_eq!(count, 0);
    }
}
