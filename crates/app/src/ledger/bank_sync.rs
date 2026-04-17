//! Bank sync engine — fetch-upsert-categorize-softmerge pipeline.

use anyhow::Result;
use chrono::{NaiveDate, Utc};
use manor_core::ledger::{bank_account, category};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::ledger::gocardless::{BankError, GoCardlessClient};

const OVERLAP_DAYS: i64 = 3;
const MIN_SYNC_INTERVAL_SECS: i64 = 5 * 60 * 60; // 5h — leaves headroom under GoCardless 4/day limit

#[derive(Debug, Serialize, Clone)]
pub struct SyncAccountReport {
    pub account_id: i64,
    pub inserted: usize,
    pub categorized: usize,
    pub merged: usize,
    pub skipped: bool,
    pub error: Option<String>,
}

pub struct SyncContext<'a> {
    pub client: &'a GoCardlessClient,
    pub allow_rate_limit_bypass: bool,
}

pub async fn sync_all(
    conn: &mut Connection,
    ctx: &SyncContext<'_>,
) -> Result<Vec<SyncAccountReport>> {
    let accounts = bank_account::list_active_for_sync(conn)?;
    let mut reports = Vec::with_capacity(accounts.len());
    for acct in accounts {
        let aid = acct.id;
        let r = sync_one(conn, ctx, aid).await;
        reports.push(r.unwrap_or_else(|e| SyncAccountReport {
            account_id: aid,
            inserted: 0,
            categorized: 0,
            merged: 0,
            skipped: false,
            error: Some(e.to_string()),
        }));
    }
    Ok(reports)
}

pub async fn sync_one(
    conn: &mut Connection,
    ctx: &SyncContext<'_>,
    account_id: i64,
) -> Result<SyncAccountReport> {
    let now = Utc::now().timestamp();
    let acct = bank_account::get(conn, account_id)?;

    // Preflight: expired requisition?
    if let Some(expires) = acct.requisition_expires_at {
        if expires < now {
            bank_account::set_sync_paused(conn, account_id, "requisition_expired")?;
            return Ok(SyncAccountReport {
                account_id,
                inserted: 0,
                categorized: 0,
                merged: 0,
                skipped: true,
                error: Some("requisition_expired".into()),
            });
        }
    }

    // Preflight: rate-limit guard.
    if !ctx.allow_rate_limit_bypass {
        if let Some(last) = acct.last_synced_at {
            if now - last < MIN_SYNC_INTERVAL_SECS {
                return Ok(SyncAccountReport {
                    account_id,
                    inserted: 0,
                    categorized: 0,
                    merged: 0,
                    skipped: true,
                    error: None,
                });
            }
        }
    }

    // Determine date_from.
    let date_from_secs = match acct.last_synced_at {
        None => {
            let days = acct.max_historical_days_granted.unwrap_or(90);
            now - days * 86_400
        }
        Some(last) => {
            let requisition_created = acct.requisition_created_at.unwrap_or(0);
            (last - OVERLAP_DAYS * 86_400).max(requisition_created)
        }
    };
    let date_from = chrono::DateTime::<Utc>::from_timestamp(date_from_secs, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "1970-01-01".into());

    // Fetch.
    let raws = match ctx.client.fetch_transactions(&acct.external_id, &date_from).await {
        Ok(v) => v,
        Err(e) => {
            if matches!(e.downcast_ref::<BankError>(), Some(BankError::RequisitionExpired)) {
                bank_account::set_sync_paused(conn, account_id, "requisition_expired")?;
                return Ok(SyncAccountReport {
                    account_id,
                    inserted: 0,
                    categorized: 0,
                    merged: 0,
                    skipped: true,
                    error: Some("requisition_expired".into()),
                });
            }
            return Err(e);
        }
    };

    // Upsert.
    let mut inserted = 0usize;
    let mut categorized = 0usize;
    for raw in &raws {
        let Some(ext_id) = raw.external_id() else {
            continue;
        };
        let Some(amount_pence) = raw.amount_pence() else {
            continue;
        };
        let booking_date = raw.booking_date_str().unwrap_or_else(|| date_from.clone());
        let date_ts = parse_date_to_ts(&booking_date).unwrap_or(now);
        let merchant = raw.merchant();
        let description = raw.description();

        let kw_text = merchant.as_deref().unwrap_or(&description);
        let kw_category = category::keyword_classify(conn, kw_text)?;

        let was_new = conn.execute(
            "INSERT INTO ledger_transaction
                (bank_account_id, external_id, amount_pence, currency, description, merchant,
                 category_id, date, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'sync', ?9)
             ON CONFLICT (bank_account_id, external_id) DO NOTHING",
            params![
                acct.id,
                ext_id,
                amount_pence,
                acct.currency,
                description,
                merchant,
                kw_category,
                date_ts,
                now,
            ],
        )?;
        if was_new > 0 {
            inserted += 1;
            if kw_category.is_some() {
                categorized += 1;
            }
        }
    }

    // First-sync soft merge — stub here, real body in Task 10.
    let merged = if acct.initial_sync_completed_at.is_none() {
        let m = soft_merge_manual_duplicates(conn, acct.id)?;
        bank_account::mark_initial_sync_completed(conn, acct.id)?;
        m
    } else {
        0
    };

    bank_account::update_sync_state(conn, acct.id, now, None)?;

    Ok(SyncAccountReport {
        account_id: acct.id,
        inserted,
        categorized,
        merged,
        skipped: false,
        error: None,
    })
}

// Body filled in Task 10. Stub returns 0.
fn soft_merge_manual_duplicates(_conn: &Connection, _account_id: i64) -> Result<usize> {
    Ok(0)
}

fn parse_date_to_ts(s: &str) -> Option<i64> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().and_then(|d| {
        d.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().timestamp())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::ledger::bank_account::InsertBankAccount;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = manor_core::assistant::db::init(&db_path).unwrap();
        (dir, conn)
    }

    fn insert_test_account(conn: &Connection, last_synced_at: Option<i64>) -> i64 {
        let a = bank_account::insert(
            conn,
            InsertBankAccount {
                provider: "gocardless",
                institution_name: "Barclays",
                institution_id: Some("BARCLAYS"),
                institution_logo_url: None,
                account_name: "Current",
                account_type: "current",
                currency: "GBP",
                external_id: "ext-1",
                requisition_id: "req-1",
                reference: "r",
                requisition_created_at: 0,
                requisition_expires_at: Utc::now().timestamp() + 100_000,
                max_historical_days_granted: 180,
            },
        )
        .unwrap();
        if let Some(ts) = last_synced_at {
            bank_account::update_sync_state(conn, a.id, ts, None).unwrap();
        }
        a.id
    }

    #[tokio::test]
    async fn rate_limit_guard_skips_recent_sync() {
        let (_dir, mut conn) = test_conn();
        let now = Utc::now().timestamp();
        let id = insert_test_account(&conn, Some(now - 60 * 60)); // 1h ago
        let client = GoCardlessClient::new("http://127.0.0.1:1");
        let ctx = SyncContext {
            client: &client,
            allow_rate_limit_bypass: false,
        };
        let report = sync_one(&mut conn, &ctx, id).await.unwrap();
        assert!(report.skipped);
        assert_eq!(report.inserted, 0);
    }

    #[tokio::test]
    async fn rate_limit_bypass_does_not_skip() {
        let (_dir, mut conn) = test_conn();
        let now = Utc::now().timestamp();
        let id = insert_test_account(&conn, Some(now - 60 * 60));
        let client = GoCardlessClient::new("http://127.0.0.1:1");
        let ctx = SyncContext {
            client: &client,
            allow_rate_limit_bypass: true,
        };
        // Will error on network; we only assert the guard didn't short-circuit.
        let res = sync_one(&mut conn, &ctx, id).await;
        match res {
            Ok(r) => assert!(!r.skipped),
            Err(_) => { /* expected — network unreachable */ }
        }
    }
}
