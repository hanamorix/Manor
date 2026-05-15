//! Approver for `add_task` proposals.
//!
//! Phase 1.D — extracted from the monolithic `proposal.rs` into a per-kind
//! module with a uniform signature that the proposal_registry (Task 1.C) will
//! dispatch into.
//!
//! ## Signature
//!
//! ```ignore
//! pub fn approve(
//!     tx: &Transaction,
//!     proposal_id: i64,
//!     diff: &str,
//!     today_override: Option<&str>,
//! ) -> Result<usize, ApplyError>
//! ```
//!
//! `today_override` is the §7-mitigated escape hatch for tests that pin a
//! deterministic "today". Production callers pass `None` and the approver
//! derives `chrono::Local::now().date_naive()` internally.
//!
//! The caller (the future proposal_registry, today the legacy shim in
//! `proposal/mod.rs`) owns the `Transaction` lifecycle and is responsible for
//! committing.

use chrono::Local;
use rusqlite::{params, Transaction};

use crate::assistant::proposal::AddTaskArgs;
use crate::assistant::proposal_error::ApplyError;
use crate::assistant::task;

/// Approve a single `add_task` proposal.
///
/// Pre-conditions (caller checks): the proposal exists, has kind `add_task`,
/// and is in `pending` state. This function does NOT re-validate kind/status —
/// that's the registry's job. It DOES parse the diff and surface
/// `ApplyError::InvalidArg` on malformed JSON.
///
/// On success: inserts the task row, marks the proposal `applied`, returns
/// `Ok(1)` (one item applied).
pub fn approve(
    tx: &Transaction,
    proposal_id: i64,
    diff: &str,
    today_override: Option<&str>,
) -> Result<usize, ApplyError> {
    let args: AddTaskArgs = serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
        field: "diff".into(),
        reason: e.to_string(),
    })?;

    let today_owned = today_override
        .map(str::to_string)
        .unwrap_or_else(|| Local::now().date_naive().format("%Y-%m-%d").to_string());
    let due_date = args.due_date.unwrap_or(today_owned);

    task::insert(tx, &args.title, Some(&due_date), Some(proposal_id))
        .map_err(|e| ApplyError::Internal(format!("task insert failed: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![now, proposal_id],
    )
    .map_err(|e| ApplyError::Internal(format!("proposal update failed: {e}")))?;

    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use crate::assistant::proposal::{insert, NewProposal};
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn make_proposal(conn: &Connection, diff: &str) -> i64 {
        insert(
            conn,
            NewProposal {
                kind: "add_task",
                rationale: "module-test",
                diff_json: diff,
                skill: "tasks",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_with_today_override_inserts_task_and_marks_applied() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({ "title": "Direct call" }).to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let n = approve(&tx, pid, &diff, Some("2026-04-15")).unwrap();
        tx.commit().unwrap();

        assert_eq!(n, 1);

        // Task row landed with the pinned today as due_date.
        let (title, due, proposal_id): (String, Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT title, due_date, proposal_id FROM task WHERE proposal_id = ?1",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(title, "Direct call");
        assert_eq!(due.as_deref(), Some("2026-04-15"));
        assert_eq!(proposal_id, Some(pid));

        // Proposal flipped to applied.
        let status: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "applied");
    }

    #[test]
    fn approve_with_none_override_falls_back_to_local_today() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({ "title": "No pinned date" }).to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        approve(&tx, pid, &diff, None).unwrap();
        tx.commit().unwrap();

        let due: Option<String> = conn
            .query_row(
                "SELECT due_date FROM task WHERE proposal_id = ?1",
                [pid],
                |r| r.get(0),
            )
            .unwrap();
        // Local::now() yields a YYYY-MM-DD string; we just assert shape.
        let s = due.expect("due_date should be populated from Local::now()");
        assert_eq!(s.len(), 10);
        assert_eq!(s.as_bytes()[4], b'-');
        assert_eq!(s.as_bytes()[7], b'-');
    }

    #[test]
    fn approve_invalid_diff_returns_invalid_arg() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_proposal(&conn, r#"{"title":"placeholder"}"#);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, "not json", Some("2026-04-15")).unwrap_err();
        match err {
            ApplyError::InvalidArg { field, .. } => assert_eq!(field, "diff"),
            other => panic!("expected InvalidArg, got {other:?}"),
        }
    }
}
