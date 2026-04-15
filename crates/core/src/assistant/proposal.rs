//! Proposals — central AI-action artefacts.
//!
//! Phase 2 scaffolded the table + types. Phase 3a wires the first lifecycle:
//! `add_task` proposals can be applied (insert task + mark applied) or
//! rejected (mark rejected, no apply).

use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::assistant::task;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Approved,
    Rejected,
    Applied,
    PartiallyApplied,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Approved => "approved",
            Status::Rejected => "rejected",
            Status::Applied => "applied",
            Status::PartiallyApplied => "partially_applied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProposal<'a> {
    pub kind: &'a str,
    pub rationale: &'a str,
    pub diff_json: &'a str,
    pub skill: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Proposal {
    pub id: i64,
    pub kind: String,
    pub rationale: String,
    pub diff: String,
    pub status: String,
    pub proposed_at: i64,
    pub applied_at: Option<i64>,
    pub skill: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTaskArgs {
    pub title: String,
    /// Optional due date. qwen2.5 occasionally emits structured objects here
    /// instead of a plain string (e.g. `{year, month, day}`). We accept any
    /// JSON shape and coerce non-strings to `None` — the caller defaults to
    /// today when `None`, which is the right v0.1 behaviour anyway.
    #[serde(default, deserialize_with = "deserialize_due_date")]
    pub due_date: Option<String>,
}

fn deserialize_due_date<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::String(s) => Ok(Some(s)),
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

/// Insert a new proposal. Returns the new row id.
pub fn insert(conn: &Connection, new: NewProposal<'_>) -> Result<i64> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO proposal (kind, rationale, diff, status, proposed_at, skill)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            new.kind,
            new.rationale,
            new.diff_json,
            Status::Pending.as_str(),
            now,
            new.skill,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// List proposals filtered by status (pass `None` for all).
pub fn list(conn: &Connection, status: Option<&str>) -> Result<Vec<Proposal>> {
    let (sql, has_filter) = match status {
        Some(_) => (
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill
             FROM proposal WHERE status = ?1 ORDER BY proposed_at",
            true,
        ),
        None => (
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill
             FROM proposal ORDER BY proposed_at",
            false,
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let mapper = |row: &rusqlite::Row| {
        Ok(Proposal {
            id: row.get("id")?,
            kind: row.get("kind")?,
            rationale: row.get("rationale")?,
            diff: row.get("diff")?,
            status: row.get("status")?,
            proposed_at: row.get("proposed_at")?,
            applied_at: row.get("applied_at")?,
            skill: row.get("skill")?,
        })
    };
    let rows: Vec<Proposal> = if has_filter {
        stmt.query_map(params![status.unwrap()], mapper)?
            .collect::<rusqlite::Result<_>>()?
    } else {
        stmt.query_map([], mapper)?
            .collect::<rusqlite::Result<_>>()?
    };
    Ok(rows)
}

/// Apply a pending `add_task` proposal: insert the task, mark proposal as `applied`.
/// Returns the refreshed list of all open tasks (caller usually wants this for UI sync).
pub fn approve_add_task(
    conn: &mut Connection,
    id: i64,
    today_iso: &str,
) -> Result<Vec<task::Task>> {
    let tx = conn.transaction()?;

    let row: Option<(String, String, String)> = tx
        .query_row(
            "SELECT kind, status, diff FROM proposal WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (kind, status, diff) = match row {
        Some(r) => r,
        None => bail!("proposal {id} not found"),
    };
    if status != "pending" {
        bail!("proposal {id} is not pending (status={status})");
    }
    if kind != "add_task" {
        bail!("proposal {id} has unsupported kind: {kind}");
    }

    let args: AddTaskArgs = serde_json::from_str(&diff)?;
    let due_date = args.due_date.unwrap_or_else(|| today_iso.to_string());
    task::insert(&tx, &args.title, Some(&due_date), Some(id))?;

    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;

    tx.commit()?;
    task::list_open(conn)
}

/// Mark a pending proposal rejected. No-op (returns Ok) if the proposal is already
/// non-pending — caller may have raced with another approve/reject.
pub fn reject(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE proposal SET status = 'rejected' WHERE id = ?1 AND status = 'pending'",
        [id],
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

    fn make_add_task_proposal(conn: &Connection, title: &str) -> i64 {
        let diff = serde_json::json!({ "title": title }).to_string();
        insert(
            conn,
            NewProposal {
                kind: "add_task",
                rationale: "test rationale",
                diff_json: &diff,
                skill: "tasks",
            },
        )
        .unwrap()
    }

    #[test]
    fn insert_returns_new_row_id() {
        let (_d, conn) = fresh_conn();
        let id = make_add_task_proposal(&conn, "Test");
        assert!(id > 0);
    }

    #[test]
    fn list_pending_filters_by_status() {
        let (_d, mut conn) = fresh_conn();
        let a = make_add_task_proposal(&conn, "A");
        let b = make_add_task_proposal(&conn, "B");
        approve_add_task(&mut conn, a, "2026-04-15").unwrap();

        let pending = list(&conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, b);

        let all = list(&conn, None).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn approve_add_task_creates_task_and_marks_applied() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Pick up prescription");
        let tasks = approve_add_task(&mut conn, pid, "2026-04-15").unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Pick up prescription");
        assert_eq!(tasks[0].due_date.as_deref(), Some("2026-04-15"));
        assert_eq!(tasks[0].proposal_id, Some(pid));

        let proposal: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(proposal, "applied");
    }

    #[test]
    fn approve_already_applied_errors() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "X");
        approve_add_task(&mut conn, pid, "2026-04-15").unwrap();
        let err = approve_add_task(&mut conn, pid, "2026-04-15").unwrap_err();
        assert!(err.to_string().contains("not pending"));
    }

    #[test]
    fn reject_marks_rejected_without_applying() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Y");
        reject(&conn, pid).unwrap();

        let proposal: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(proposal, "rejected");

        let task_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task", [], |r| r.get(0))
            .unwrap();
        assert_eq!(task_count, 0);
    }

    #[test]
    fn reject_already_rejected_is_noop() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Z");
        reject(&conn, pid).unwrap();
        reject(&conn, pid).unwrap(); // does not error
    }

    #[test]
    fn approve_uses_proposal_due_date_when_present() {
        let (_d, mut conn) = fresh_conn();
        let diff =
            serde_json::json!({ "title": "Future thing", "due_date": "2026-04-30" }).to_string();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_task",
                rationale: "r",
                diff_json: &diff,
                skill: "tasks",
            },
        )
        .unwrap();
        let tasks = approve_add_task(&mut conn, pid, "2026-04-15").unwrap();
        assert_eq!(tasks[0].due_date.as_deref(), Some("2026-04-30"));
    }

    #[test]
    fn add_task_args_coerces_non_string_due_date_to_none() {
        // qwen2.5 sometimes emits due_date as a structured object instead of
        // a string. We accept the arguments and drop the weird due_date rather
        // than failing the whole deserialize.
        let weird = serde_json::json!({
            "title": "Book a dentist appointment",
            "due_date": { "year": 2026, "month": 4, "day": 20 }
        });
        let parsed: AddTaskArgs = serde_json::from_value(weird).unwrap();
        assert_eq!(parsed.title, "Book a dentist appointment");
        assert_eq!(parsed.due_date, None);
    }
}
