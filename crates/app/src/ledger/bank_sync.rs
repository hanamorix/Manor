//! Bank sync engine — fetch-upsert-categorize-softmerge pipeline.

use anyhow::Result;
use chrono::{NaiveDate, Utc};
use manor_core::assistant::proposal::{self, NewProposal};
use manor_core::ledger::{bank_account, category};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::ledger::gocardless::{BankError, GoCardlessClient};

const OVERLAP_DAYS: i64 = 3;
const MIN_SYNC_INTERVAL_SECS: i64 = 5 * 60 * 60; // 5h — leaves headroom under GoCardless 4/day limit
const NUDGE_DEDUP_SECS: i64 = 24 * 60 * 60; // no more than one bank_reconnect bubble per 24h

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
            maybe_nudge_reconnect(conn, &acct, now)?;
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

/// Inserts a `bank_reconnect` proposal so the assistant bubble flow can
/// surface it. Dedups via `bank_account.last_nudge_at` — at most one nudge
/// per 24h so a stuck expired account doesn't spam on every sync tick.
fn maybe_nudge_reconnect(
    conn: &Connection,
    acct: &bank_account::BankAccount,
    now: i64,
) -> Result<()> {
    if let Some(last) = acct.last_nudge_at {
        if now - last < NUDGE_DEDUP_SECS {
            return Ok(());
        }
    }

    let rationale = format!(
        "Your {} link has expired — reconnect to resume sync.",
        acct.institution_name
    );
    let diff_json = serde_json::json!({
        "action": "bank_reconnect",
        "account_id": acct.id,
        "institution_name": acct.institution_name,
    })
    .to_string();

    proposal::insert(
        conn,
        NewProposal {
            kind: "bank_reconnect",
            rationale: &rationale,
            diff_json: &diff_json,
            skill: "bank_sync",
        },
    )?;
    bank_account::set_last_nudge_at(conn, acct.id, now)?;
    Ok(())
}

/// Runs once per account on first sync. Finds manual rows with matching
/// amount + date (±1 day) and:
///   - copies manual.category_id to bank row if bank has none,
///   - copies manual.note to bank row (appending if both exist),
///   - soft-deletes the manual row.
fn soft_merge_manual_duplicates(conn: &Connection, account_id: i64) -> Result<usize> {
    #[derive(Debug)]
    struct Pair {
        manual_id: i64,
        bank_id: i64,
        manual_category_id: Option<i64>,
        manual_note: Option<String>,
        bank_category_id: Option<i64>,
        bank_note: Option<String>,
    }
    let mut stmt = conn.prepare(
        "SELECT m.id AS manual_id, b.id AS bank_id,
                m.category_id AS manual_category_id, m.note AS manual_note,
                b.category_id AS bank_category_id, b.note AS bank_note
         FROM ledger_transaction m
         JOIN ledger_transaction b ON (
               b.bank_account_id = ?1
           AND b.source = 'sync'
           AND m.bank_account_id IS NULL
           AND m.source = 'manual'
           AND m.amount_pence = b.amount_pence
           AND ABS(m.date - b.date) <= 86400
         )
         WHERE m.deleted_at IS NULL AND b.deleted_at IS NULL",
    )?;
    let pairs: Vec<Pair> = stmt
        .query_map(params![account_id], |row| {
            Ok(Pair {
                manual_id: row.get("manual_id")?,
                bank_id: row.get("bank_id")?,
                manual_category_id: row.get("manual_category_id")?,
                manual_note: row.get("manual_note")?,
                bank_category_id: row.get("bank_category_id")?,
                bank_note: row.get("bank_note")?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let now = Utc::now().timestamp();
    let mut merged = 0usize;
    for p in pairs {
        let new_category = p.bank_category_id.or(p.manual_category_id);
        let new_note = match (p.bank_note, p.manual_note) {
            (None, None) => None,
            (Some(b), None) => Some(b),
            (None, Some(m)) => Some(m),
            (Some(b), Some(m)) if b == m => Some(b),
            (Some(b), Some(m)) => Some(format!("{b}\n{m}")),
        };
        conn.execute(
            "UPDATE ledger_transaction SET category_id = ?1, note = ?2 WHERE id = ?3",
            params![new_category, new_note, p.bank_id],
        )?;
        conn.execute(
            "UPDATE ledger_transaction SET deleted_at = ?1 WHERE id = ?2",
            params![now, p.manual_id],
        )?;
        merged += 1;
    }
    Ok(merged)
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

    #[test]
    fn soft_merge_preserves_manual_category_and_note() {
        let (_dir, conn) = test_conn();
        let acct_id = insert_test_account(&conn, None);

        // Insert a manual row: £12.40 on 2026-04-10, category 3, note "on the way".
        let date = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
            .and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        conn.execute(
            "INSERT INTO ledger_transaction
                (bank_account_id, amount_pence, currency, description, category_id, date, source, note)
             VALUES (NULL, -1240, 'GBP', 'tesco', 3, ?1, 'manual', 'on the way')",
            params![date],
        ).unwrap();

        // Insert the bank-synced row: same amount, same day, no category, no note.
        conn.execute(
            "INSERT INTO ledger_transaction
                (bank_account_id, external_id, amount_pence, currency, description, merchant, date, source)
             VALUES (?1, 'tx-1', -1240, 'GBP', 'TESCO STORES 4023', 'TESCO', ?2, 'sync')",
            params![acct_id, date],
        ).unwrap();

        let merged = soft_merge_manual_duplicates(&conn, acct_id).unwrap();
        assert_eq!(merged, 1);

        // Manual row soft-deleted.
        let manual_deleted: i64 = conn.query_row(
            "SELECT deleted_at IS NOT NULL FROM ledger_transaction
             WHERE source = 'manual' AND bank_account_id IS NULL",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(manual_deleted, 1);

        // Bank row carries category 3 and note "on the way".
        let (cat, note): (Option<i64>, Option<String>) = conn.query_row(
            "SELECT category_id, note FROM ledger_transaction WHERE external_id = 'tx-1'",
            [], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(cat, Some(3));
        assert_eq!(note, Some("on the way".into()));
    }

    #[test]
    fn soft_merge_keeps_bank_category_when_set() {
        let (_dir, conn) = test_conn();
        let acct_id = insert_test_account(&conn, None);
        let date = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO ledger_transaction
                (bank_account_id, amount_pence, currency, description, category_id, date, source, note)
             VALUES (NULL, -500, 'GBP', 'coffee', 2, ?1, 'manual', NULL)",
            params![date],
        ).unwrap();
        conn.execute(
            "INSERT INTO ledger_transaction
                (bank_account_id, external_id, amount_pence, currency, description, category_id, date, source)
             VALUES (?1, 'tx-2', -500, 'GBP', 'COSTA', 1, ?2, 'sync')",
            params![acct_id, date],
        ).unwrap();

        soft_merge_manual_duplicates(&conn, acct_id).unwrap();

        let cat: Option<i64> = conn.query_row(
            "SELECT category_id FROM ledger_transaction WHERE external_id = 'tx-2'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(cat, Some(1)); // bank_category_id wins since it was set
    }

    #[test]
    fn nudge_inserts_proposal_and_dedups_within_24h() {
        let (_dir, conn) = test_conn();
        let acct_id = insert_test_account(&conn, None);
        let acct = bank_account::get(&conn, acct_id).unwrap();
        let now = Utc::now().timestamp();

        // First nudge — inserts.
        maybe_nudge_reconnect(&conn, &acct, now).unwrap();
        let count1: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal WHERE kind = 'bank_reconnect'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count1, 1);

        // Second nudge <24h later — deduped.
        let acct_after = bank_account::get(&conn, acct_id).unwrap();
        maybe_nudge_reconnect(&conn, &acct_after, now + 3600).unwrap();
        let count2: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal WHERE kind = 'bank_reconnect'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count2, 1);

        // Third nudge >24h later — fires again.
        let acct_after = bank_account::get(&conn, acct_id).unwrap();
        maybe_nudge_reconnect(&conn, &acct_after, now + NUDGE_DEDUP_SECS + 1).unwrap();
        let count3: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposal WHERE kind = 'bank_reconnect'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count3, 2);
    }
}
