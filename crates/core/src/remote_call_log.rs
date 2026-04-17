//! Audit log DAL for remote LLM calls. See V11 migration for schema.

use anyhow::Result;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallLogEntry {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub skill: String,
    pub user_visible_reason: String,
    pub prompt_redacted: String,
    pub response_text: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cost_pence: Option<i64>,
    pub redaction_count: i64,
    pub error: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
}

impl CallLogEntry {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            provider: row.get("provider")?,
            model: row.get("model")?,
            skill: row.get("skill")?,
            user_visible_reason: row.get("user_visible_reason")?,
            prompt_redacted: row.get("prompt_redacted")?,
            response_text: row.get("response_text")?,
            input_tokens: row.get("input_tokens")?,
            output_tokens: row.get("output_tokens")?,
            cost_pence: row.get("cost_pence")?,
            redaction_count: row.get("redaction_count")?,
            error: row.get("error")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
        })
    }
}

pub struct NewCall<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub skill: &'a str,
    pub user_visible_reason: &'a str,
    pub prompt_redacted: &'a str,
    pub redaction_count: i64,
}

/// Insert an in-flight row. Returns the id for later mark_completed/mark_errored.
pub fn insert_started(conn: &Connection, new: NewCall<'_>) -> Result<i64> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO remote_call_log
         (provider, model, skill, user_visible_reason, prompt_redacted,
          redaction_count, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            new.provider,
            new.model,
            new.skill,
            new.user_visible_reason,
            new.prompt_redacted,
            new.redaction_count,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn mark_completed(
    conn: &Connection,
    id: i64,
    response_text: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost_pence: i64,
) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE remote_call_log
         SET response_text = ?1, input_tokens = ?2, output_tokens = ?3,
             cost_pence = ?4, completed_at = ?5
         WHERE id = ?6",
        params![
            response_text,
            input_tokens,
            output_tokens,
            cost_pence,
            now,
            id
        ],
    )?;
    Ok(())
}

pub fn mark_errored(conn: &Connection, id: i64, error: &str) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE remote_call_log
         SET error = ?1, completed_at = ?2
         WHERE id = ?3",
        params![error, now, id],
    )?;
    Ok(())
}

pub fn list_recent(conn: &Connection, limit: usize) -> Result<Vec<CallLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, model, skill, user_visible_reason, prompt_redacted,
                response_text, input_tokens, output_tokens, cost_pence,
                redaction_count, error, started_at, completed_at
         FROM remote_call_log
         WHERE deleted_at IS NULL
         ORDER BY started_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit as i64], CallLogEntry::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Sum `cost_pence` for all non-deleted calls in the current calendar month (UTC)
/// for a given provider. Returns 0 if none.
pub fn sum_month_pence(conn: &Connection, provider: &str, now: DateTime<Utc>) -> Result<i64> {
    let month_start = Utc
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid month start"))?
        .timestamp();
    let (next_year, next_month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let month_end = Utc
        .with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid month end"))?
        .timestamp();

    let total: i64 = conn.query_row(
        "SELECT COALESCE(SUM(cost_pence), 0)
         FROM remote_call_log
         WHERE provider = ?1
           AND deleted_at IS NULL
           AND started_at >= ?2 AND started_at < ?3",
        params![provider, month_start, month_end],
        |r| r.get(0),
    )?;
    Ok(total)
}

/// Soft-delete every row (user clicks "Clear call log").
pub fn clear_all(conn: &Connection) -> Result<usize> {
    let now = Utc::now().timestamp();
    let n = conn.execute(
        "UPDATE remote_call_log SET deleted_at = ?1 WHERE deleted_at IS NULL",
        params![now],
    )?;
    Ok(n)
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

    fn sample<'a>() -> NewCall<'a> {
        NewCall {
            provider: "claude",
            model: "claude-sonnet-4-6",
            skill: "ledger_review",
            user_visible_reason: "Write April spending narrative",
            prompt_redacted: "You are a calm personal finance assistant...",
            redaction_count: 0,
        }
    }

    #[test]
    fn insert_started_returns_id_and_creates_in_flight_row() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        assert!(id > 0);
        let entries = list_recent(&conn, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].response_text.is_none());
        assert!(entries[0].completed_at.is_none());
        assert!(entries[0].error.is_none());
    }

    #[test]
    fn mark_completed_fills_response_and_tokens() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        mark_completed(&conn, id, "Your April was calm.", 300, 80, 25).unwrap();
        let e = list_recent(&conn, 10).unwrap().into_iter().next().unwrap();
        assert_eq!(e.response_text.as_deref(), Some("Your April was calm."));
        assert_eq!(e.input_tokens, Some(300));
        assert_eq!(e.output_tokens, Some(80));
        assert_eq!(e.cost_pence, Some(25));
        assert!(e.completed_at.is_some());
    }

    #[test]
    fn mark_errored_sets_error_and_completed() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        mark_errored(&conn, id, "timeout after 3 attempts").unwrap();
        let e = list_recent(&conn, 10).unwrap().into_iter().next().unwrap();
        assert_eq!(e.error.as_deref(), Some("timeout after 3 attempts"));
        assert!(e.completed_at.is_some());
        assert!(e.response_text.is_none());
    }

    #[test]
    fn list_recent_orders_newest_first_and_respects_limit() {
        let (_d, conn) = fresh_conn();
        for _ in 0..5 {
            insert_started(&conn, sample()).unwrap();
        }
        let entries = list_recent(&conn, 3).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].started_at >= entries[1].started_at);
    }

    #[test]
    fn sum_month_pence_aggregates_only_current_month() {
        let (_d, conn) = fresh_conn();
        let id = insert_started(&conn, sample()).unwrap();
        mark_completed(&conn, id, "ok", 100, 50, 15).unwrap();

        // Plant a row stamped last month.
        let last_month = Utc
            .with_ymd_and_hms(2026, 3, 15, 12, 0, 0)
            .unwrap()
            .timestamp();
        conn.execute(
            "INSERT INTO remote_call_log
             (provider, model, skill, user_visible_reason, prompt_redacted,
              redaction_count, started_at, completed_at, cost_pence)
             VALUES ('claude','claude-sonnet-4-6','ledger_review','old','x', 0, ?1, ?1, 999)",
            [last_month],
        )
        .unwrap();

        let now = Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();
        let total = sum_month_pence(&conn, "claude", now).unwrap();
        assert_eq!(total, 15, "last month's 999 must not count");
    }

    #[test]
    fn sum_month_pence_filters_by_provider() {
        let (_d, conn) = fresh_conn();
        let claude_id = insert_started(&conn, sample()).unwrap();
        mark_completed(&conn, claude_id, "ok", 100, 50, 20).unwrap();

        let openai_id = insert_started(
            &conn,
            NewCall {
                provider: "openai",
                ..sample()
            },
        )
        .unwrap();
        mark_completed(&conn, openai_id, "ok", 100, 50, 5).unwrap();

        let now = Utc::now();
        assert_eq!(sum_month_pence(&conn, "claude", now).unwrap(), 20);
        assert_eq!(sum_month_pence(&conn, "openai", now).unwrap(), 5);
    }

    #[test]
    fn clear_all_soft_deletes_every_row() {
        let (_d, conn) = fresh_conn();
        insert_started(&conn, sample()).unwrap();
        insert_started(&conn, sample()).unwrap();
        let n = clear_all(&conn).unwrap();
        assert_eq!(n, 2);
        assert!(list_recent(&conn, 10).unwrap().is_empty());
    }
}
