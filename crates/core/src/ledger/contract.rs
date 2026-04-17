//! Contract DAL — service contracts with renewal alerts.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Contract {
    pub id: i64,
    pub provider: String,
    pub kind: String,
    pub description: Option<String>,
    pub monthly_cost_pence: i64,
    pub term_start: i64,
    pub term_end: i64,
    pub exit_fee_pence: Option<i64>,
    pub renewal_alert_days: i64,
    pub recurring_payment_id: Option<i64>,
    pub note: Option<String>,
    pub created_at: i64,
}

impl Contract {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            provider: row.get("provider")?,
            kind: row.get("kind")?,
            description: row.get("description")?,
            monthly_cost_pence: row.get("monthly_cost_pence")?,
            term_start: row.get("term_start")?,
            term_end: row.get("term_end")?,
            exit_fee_pence: row.get("exit_fee_pence")?,
            renewal_alert_days: row.get("renewal_alert_days")?,
            recurring_payment_id: row.get("recurring_payment_id")?,
            note: row.get("note")?,
            created_at: row.get("created_at")?,
        })
    }
}

pub struct NewContract<'a> {
    pub provider: &'a str,
    pub kind: &'a str,
    pub description: Option<&'a str>,
    pub monthly_cost_pence: i64,
    pub term_start: i64,
    pub term_end: i64,
    pub exit_fee_pence: Option<i64>,
    pub renewal_alert_days: i64,
    pub recurring_payment_id: Option<i64>,
    pub note: Option<&'a str>,
}

pub fn insert(conn: &Connection, new: NewContract<'_>) -> Result<Contract> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO contract
         (provider, kind, description, monthly_cost_pence, term_start, term_end,
          exit_fee_pence, renewal_alert_days, recurring_payment_id, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            new.provider,
            new.kind,
            new.description,
            new.monthly_cost_pence,
            new.term_start,
            new.term_end,
            new.exit_fee_pence,
            new.renewal_alert_days,
            new.recurring_payment_id,
            new.note,
            now
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn list(conn: &Connection) -> Result<Vec<Contract>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, kind, description, monthly_cost_pence, term_start,
                term_end, exit_fee_pence, renewal_alert_days, recurring_payment_id,
                note, created_at
         FROM contract
         WHERE deleted_at IS NULL
         ORDER BY term_end ASC",
    )?;
    let result = stmt
        .query_map([], Contract::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(result)
}

pub fn update(conn: &Connection, id: i64, new: NewContract<'_>) -> Result<Contract> {
    let rows = conn.execute(
        "UPDATE contract
         SET provider = ?1, kind = ?2, description = ?3, monthly_cost_pence = ?4,
             term_start = ?5, term_end = ?6, exit_fee_pence = ?7,
             renewal_alert_days = ?8, recurring_payment_id = ?9, note = ?10
         WHERE id = ?11 AND deleted_at IS NULL",
        params![
            new.provider,
            new.kind,
            new.description,
            new.monthly_cost_pence,
            new.term_start,
            new.term_end,
            new.exit_fee_pence,
            new.renewal_alert_days,
            new.recurring_payment_id,
            new.note,
            id
        ],
    )?;
    anyhow::ensure!(rows > 0, "contract id={id} not found");
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE contract SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenewalAlert {
    pub contract_id: i64,
    pub provider: String,
    pub kind: String,
    pub term_end: i64,
    pub days_remaining: i64,
    pub exit_fee_pence: Option<i64>,
    pub severity: String, // "amber" or "red"
}

/// Return contracts whose `term_end` is within their `renewal_alert_days` window.
/// `severity = "red"` if ≤7 days remaining, else `"amber"`.
pub fn check_renewals(conn: &Connection, now_ts: i64) -> Result<Vec<RenewalAlert>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, kind, term_end, renewal_alert_days, exit_fee_pence
         FROM contract
         WHERE deleted_at IS NULL
           AND term_end - ?1 <= renewal_alert_days * 86400
           AND term_end > ?1
         ORDER BY term_end ASC",
    )?;
    let rows = stmt
        .query_map(params![now_ts], |row| {
            let id: i64 = row.get(0)?;
            let provider: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let term_end: i64 = row.get(3)?;
            let _alert_days: i64 = row.get(4)?;
            let exit_fee: Option<i64> = row.get(5)?;
            let days_remaining = ((term_end - now_ts).max(0)) / 86400;
            let severity = if days_remaining <= 7 { "red" } else { "amber" };
            Ok(RenewalAlert {
                contract_id: id,
                provider,
                kind,
                term_end,
                days_remaining,
                exit_fee_pence: exit_fee,
                severity: severity.to_string(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn get(conn: &Connection, id: i64) -> Result<Contract> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, kind, description, monthly_cost_pence, term_start,
                term_end, exit_fee_pence, renewal_alert_days, recurring_payment_id,
                note, created_at
         FROM contract WHERE id = ?1",
    )?;
    Ok(stmt.query_row([id], Contract::from_row)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn ts(year: i32, month: u32, day: u32) -> i64 {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0)
            .unwrap()
            .timestamp()
    }

    fn sample(provider: &'static str, term_end: i64) -> NewContract<'static> {
        NewContract {
            provider,
            kind: "phone",
            description: None,
            monthly_cost_pence: 2500,
            term_start: ts(2025, 1, 1),
            term_end,
            exit_fee_pence: None,
            renewal_alert_days: 30,
            recurring_payment_id: None,
            note: None,
        }
    }

    #[test]
    fn insert_and_list() {
        let (_d, conn) = fresh_conn();
        insert(&conn, sample("O2", ts(2027, 1, 1))).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn list_orders_by_term_end() {
        let (_d, conn) = fresh_conn();
        insert(&conn, sample("Later", ts(2027, 6, 1))).unwrap();
        insert(&conn, sample("Sooner", ts(2026, 6, 1))).unwrap();
        let rows = list(&conn).unwrap();
        assert_eq!(rows[0].provider, "Sooner");
        assert_eq!(rows[1].provider, "Later");
    }

    #[test]
    fn update_changes_fields() {
        let (_d, conn) = fresh_conn();
        let c = insert(&conn, sample("O2", ts(2027, 1, 1))).unwrap();
        let changed = update(
            &conn,
            c.id,
            NewContract {
                provider: "EE",
                kind: "phone",
                description: None,
                monthly_cost_pence: 3000,
                term_start: c.term_start,
                term_end: ts(2028, 1, 1),
                exit_fee_pence: Some(5000),
                renewal_alert_days: 60,
                recurring_payment_id: None,
                note: None,
            },
        )
        .unwrap();
        assert_eq!(changed.provider, "EE");
        assert_eq!(changed.monthly_cost_pence, 3000);
        assert_eq!(changed.exit_fee_pence, Some(5000));
    }

    #[test]
    fn delete_soft_deletes() {
        let (_d, conn) = fresh_conn();
        let c = insert(&conn, sample("O2", ts(2027, 1, 1))).unwrap();
        delete(&conn, c.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }

    fn within(new_term_end: i64, alert_days: i64) -> NewContract<'static> {
        NewContract {
            provider: "O2",
            kind: "phone",
            description: None,
            monthly_cost_pence: 2500,
            term_start: ts(2025, 1, 1),
            term_end: new_term_end,
            exit_fee_pence: None,
            renewal_alert_days: alert_days,
            recurring_payment_id: None,
            note: None,
        }
    }

    #[test]
    fn check_renewals_includes_contracts_inside_alert_window() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        insert(&conn, within(now + 20 * 86400, 30)).unwrap(); // 20 days away, 30-day window
        insert(&conn, within(now + 90 * 86400, 30)).unwrap(); // 90 days away — out of window
        let alerts = check_renewals(&conn, now).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].days_remaining, 20);
        assert_eq!(alerts[0].severity, "amber");
    }

    #[test]
    fn check_renewals_marks_red_under_seven_days() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        insert(&conn, within(now + 3 * 86400, 30)).unwrap();
        let alerts = check_renewals(&conn, now).unwrap();
        assert_eq!(alerts[0].severity, "red");
    }

    #[test]
    fn check_renewals_excludes_already_expired() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        insert(&conn, within(now - 5 * 86400, 30)).unwrap();
        assert!(check_renewals(&conn, now).unwrap().is_empty());
    }

    #[test]
    fn check_renewals_excludes_deleted() {
        let (_d, conn) = fresh_conn();
        let now = ts(2026, 4, 16);
        let c = insert(&conn, within(now + 5 * 86400, 30)).unwrap();
        delete(&conn, c.id).unwrap();
        assert!(check_renewals(&conn, now).unwrap().is_empty());
    }
}
