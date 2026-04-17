//! Bank account DAL. Rows are created by GoCardless connect flow in `manor-app`.
//! Core never makes HTTP calls — it only reads/writes the row.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankAccount {
    pub id: i64,
    pub provider: String,
    pub institution_name: String,
    pub institution_id: Option<String>,
    pub institution_logo_url: Option<String>,
    pub account_name: String,
    pub account_type: String,
    pub currency: String,
    pub external_id: String,
    pub requisition_id: Option<String>,
    pub reference: Option<String>,
    pub requisition_created_at: Option<i64>,
    pub requisition_expires_at: Option<i64>,
    pub max_historical_days_granted: Option<i64>,
    pub last_synced_at: Option<i64>,
    pub last_nudge_at: Option<i64>,
    pub sync_paused_reason: Option<String>,
    pub initial_sync_completed_at: Option<i64>,
    pub created_at: i64,
}

impl BankAccount {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            provider: row.get("provider")?,
            institution_name: row.get("institution_name")?,
            institution_id: row.get("institution_id")?,
            institution_logo_url: row.get("institution_logo_url")?,
            account_name: row.get("account_name")?,
            account_type: row.get("account_type")?,
            currency: row.get("currency")?,
            external_id: row.get("external_id")?,
            requisition_id: row.get("requisition_id")?,
            reference: row.get("reference")?,
            requisition_created_at: row.get("requisition_created_at")?,
            requisition_expires_at: row.get("requisition_expires_at")?,
            max_historical_days_granted: row.get("max_historical_days_granted")?,
            last_synced_at: row.get("last_synced_at")?,
            last_nudge_at: row.get("last_nudge_at")?,
            sync_paused_reason: row.get("sync_paused_reason")?,
            initial_sync_completed_at: row.get("initial_sync_completed_at")?,
            created_at: row.get("created_at")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct InsertBankAccount<'a> {
    pub provider: &'a str,
    pub institution_name: &'a str,
    pub institution_id: Option<&'a str>,
    pub institution_logo_url: Option<&'a str>,
    pub account_name: &'a str,
    pub account_type: &'a str,
    pub currency: &'a str,
    pub external_id: &'a str,
    pub requisition_id: &'a str,
    pub reference: &'a str,
    pub requisition_created_at: i64,
    pub requisition_expires_at: i64,
    pub max_historical_days_granted: i64,
}

pub fn insert(conn: &Connection, row: InsertBankAccount<'_>) -> Result<BankAccount> {
    conn.execute(
        "INSERT INTO bank_account (
             provider, institution_name, institution_id, institution_logo_url,
             account_name, account_type, currency, external_id,
             requisition_id, reference, requisition_created_at, requisition_expires_at,
             max_historical_days_granted, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            row.provider, row.institution_name, row.institution_id, row.institution_logo_url,
            row.account_name, row.account_type, row.currency, row.external_id,
            row.requisition_id, row.reference, row.requisition_created_at,
            row.requisition_expires_at, row.max_historical_days_granted,
            Utc::now().timestamp(),
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

const SELECT_COLS: &str =
    "id, provider, institution_name, institution_id, institution_logo_url, \
     account_name, account_type, currency, external_id, requisition_id, reference, \
     requisition_created_at, requisition_expires_at, max_historical_days_granted, \
     last_synced_at, last_nudge_at, sync_paused_reason, initial_sync_completed_at, created_at";

pub fn get(conn: &Connection, id: i64) -> Result<BankAccount> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM bank_account WHERE id = ?1 AND deleted_at IS NULL"
    );
    Ok(conn.query_row(&sql, [id], BankAccount::from_row)?)
}

pub fn list(conn: &Connection) -> Result<Vec<BankAccount>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM bank_account WHERE deleted_at IS NULL ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], BankAccount::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_active_for_sync(conn: &Connection) -> Result<Vec<BankAccount>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM bank_account \
         WHERE deleted_at IS NULL AND sync_paused_reason IS NULL \
         ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], BankAccount::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn update_sync_state(
    conn: &Connection,
    id: i64,
    last_synced_at: i64,
    sync_paused_reason: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE bank_account SET last_synced_at = ?1, sync_paused_reason = ?2
         WHERE id = ?3",
        params![last_synced_at, sync_paused_reason, id],
    )?;
    Ok(())
}

pub fn mark_initial_sync_completed(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE bank_account SET initial_sync_completed_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;
    Ok(())
}

pub fn set_sync_paused(conn: &Connection, id: i64, reason: &str) -> Result<()> {
    conn.execute(
        "UPDATE bank_account SET sync_paused_reason = ?1 WHERE id = ?2",
        params![reason, id],
    )?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE bank_account SET deleted_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;
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
    fn insert_list_get_roundtrip() {
        let (_d, conn) = fresh_conn();
        let inserted = insert(&conn, InsertBankAccount {
            provider: "gocardless",
            institution_name: "Barclays",
            institution_id: Some("BARCLAYS_GB_BUKBGB22"),
            institution_logo_url: Some("https://cdn.gocardless.com/barclays.png"),
            account_name: "Current",
            account_type: "current",
            currency: "GBP",
            external_id: "acc-abc",
            requisition_id: "req-xyz",
            reference: "uuid-1",
            requisition_created_at: 1_700_000_000,
            requisition_expires_at: 1_715_000_000,
            max_historical_days_granted: 180,
        }).unwrap();

        assert_eq!(inserted.institution_name, "Barclays");
        assert_eq!(inserted.max_historical_days_granted, Some(180));

        let listed = list(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, inserted.id);

        let fetched = get(&conn, inserted.id).unwrap();
        assert_eq!(fetched, inserted);
    }

    #[test]
    fn list_active_for_sync_excludes_paused_and_deleted() {
        let (_d, conn) = fresh_conn();
        let a = insert(&conn, fixture("a", "acc-1")).unwrap();
        let b = insert(&conn, fixture("b", "acc-2")).unwrap();
        let c = insert(&conn, fixture("c", "acc-3")).unwrap();

        set_sync_paused(&conn, b.id, "requisition_expired").unwrap();
        soft_delete(&conn, c.id).unwrap();

        let active = list_active_for_sync(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, a.id);
    }

    #[test]
    fn update_sync_state_clears_pause() {
        let (_d, conn) = fresh_conn();
        let acct = insert(&conn, fixture("a", "acc-1")).unwrap();
        set_sync_paused(&conn, acct.id, "requisition_expired").unwrap();
        update_sync_state(&conn, acct.id, 1_800_000_000, None).unwrap();
        let fetched = get(&conn, acct.id).unwrap();
        assert_eq!(fetched.sync_paused_reason, None);
        assert_eq!(fetched.last_synced_at, Some(1_800_000_000));
    }

    fn fixture<'a>(name: &'a str, ext: &'a str) -> InsertBankAccount<'a> {
        InsertBankAccount {
            provider: "gocardless", institution_name: name,
            institution_id: None, institution_logo_url: None,
            account_name: "Current", account_type: "current", currency: "GBP",
            external_id: ext, requisition_id: "req", reference: "ref",
            requisition_created_at: 0, requisition_expires_at: 0,
            max_historical_days_granted: 180,
        }
    }
}
