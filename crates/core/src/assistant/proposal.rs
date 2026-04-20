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
pub struct AddMaintenanceScheduleArgs {
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub notes: String,
    pub source_attachment_uuid: String,
    pub tier: String,
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

/// Apply a pending `add_maintenance_schedule` proposal verbatim.
/// Inserts the schedule + marks the proposal `applied`.
/// Returns the inserted schedule's id.
pub fn approve_add_maintenance_schedule(conn: &mut Connection, id: i64) -> Result<String> {
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
    if kind != "add_maintenance_schedule" {
        bail!("proposal {id} has unsupported kind: {kind}");
    }

    let args: AddMaintenanceScheduleArgs = serde_json::from_str(&diff)?;
    let draft = crate::maintenance::MaintenanceScheduleDraft {
        asset_id: args.asset_id,
        task: args.task,
        interval_months: args.interval_months,
        last_done_date: None,
        notes: args.notes,
    };
    let schedule_id = crate::maintenance::dal::insert_schedule(&tx, &draft)?;

    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;

    tx.commit()?;
    Ok(schedule_id)
}

/// Apply a pending `add_maintenance_schedule` proposal using caller-supplied
/// edited fields (overrides the diff's values). Used by ScheduleDrawer in
/// proposal-edit mode. Returns the inserted schedule's id.
pub fn approve_add_maintenance_schedule_with_override(
    conn: &mut Connection,
    id: i64,
    edited: &crate::maintenance::MaintenanceScheduleDraft,
) -> Result<String> {
    let tx = conn.transaction()?;

    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT kind, status FROM proposal WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (kind, status) = match row {
        Some(r) => r,
        None => bail!("proposal {id} not found"),
    };
    if kind != "add_maintenance_schedule" {
        bail!("proposal {id} is not an add_maintenance_schedule");
    }
    if status != "pending" {
        bail!("proposal {id} is not pending (status={status})");
    }

    let schedule_id = crate::maintenance::dal::insert_schedule(&tx, edited)?;

    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![Utc::now().timestamp(), id],
    )?;

    tx.commit()?;
    Ok(schedule_id)
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

    // ── L4e add_maintenance_schedule proposals ────────────────────────────

    fn insert_test_asset(conn: &Connection) -> String {
        use crate::asset::{dal as asset_dal, AssetCategory, AssetDraft};
        asset_dal::insert_asset(
            conn,
            &AssetDraft {
                name: "Boiler".into(),
                category: AssetCategory::Appliance,
                make: None,
                model: None,
                serial_number: None,
                purchase_date: None,
                notes: String::new(),
                hero_attachment_uuid: None,
            },
        )
        .unwrap()
    }

    fn insert_pending_schedule_proposal(
        conn: &Connection,
        asset_id: &str,
        task: &str,
        interval_months: i32,
        source_attachment_uuid: &str,
    ) -> i64 {
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.into(),
            task: task.into(),
            interval_months,
            notes: String::new(),
            source_attachment_uuid: source_attachment_uuid.into(),
            tier: "ollama".into(),
        };
        let diff = serde_json::to_string(&args).unwrap();
        insert(
            conn,
            NewProposal {
                kind: "add_maintenance_schedule",
                rationale: "test",
                diff_json: &diff,
                skill: "pdf_extract",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_add_maintenance_schedule_inserts_and_marks_applied() {
        let (_d, mut conn) = fresh_conn();
        let asset_id = insert_test_asset(&conn);
        let pid = insert_pending_schedule_proposal(&conn, &asset_id, "Annual service", 12, "att-uuid-1");

        let schedule_id = approve_add_maintenance_schedule(&mut conn, pid).unwrap();
        assert!(!schedule_id.is_empty());

        let s = crate::maintenance::dal::get_schedule(&conn, &schedule_id)
            .unwrap()
            .unwrap();
        assert_eq!(s.task, "Annual service");
        assert_eq!(s.interval_months, 12);

        let (status, applied_at): (String, Option<i64>) = conn
            .query_row(
                "SELECT status, applied_at FROM proposal WHERE id = ?1",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "applied");
        assert!(applied_at.is_some());
    }

    #[test]
    fn approve_add_maintenance_schedule_fails_on_non_pending() {
        let (_d, mut conn) = fresh_conn();
        let asset_id = insert_test_asset(&conn);
        let pid = insert_pending_schedule_proposal(&conn, &asset_id, "Service", 12, "att");
        approve_add_maintenance_schedule(&mut conn, pid).unwrap();
        let err = approve_add_maintenance_schedule(&mut conn, pid)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not pending"), "got: {}", err);
    }

    #[test]
    fn approve_add_maintenance_schedule_fails_on_wrong_kind() {
        let (_d, mut conn) = fresh_conn();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_task",
                rationale: "r",
                diff_json: r#"{"title":"t","due_date":null}"#,
                skill: "test",
            },
        )
        .unwrap();
        let err = approve_add_maintenance_schedule(&mut conn, pid)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unsupported kind"), "got: {}", err);
    }

    #[test]
    fn approve_with_override_uses_edited_fields() {
        let (_d, mut conn) = fresh_conn();
        let asset_id = insert_test_asset(&conn);
        let pid = insert_pending_schedule_proposal(&conn, &asset_id, "Original", 12, "att");

        let edited = crate::maintenance::MaintenanceScheduleDraft {
            asset_id: asset_id.clone(),
            task: "Edited task".into(),
            interval_months: 24,
            last_done_date: None,
            notes: "edited notes".into(),
        };
        let sched_id =
            approve_add_maintenance_schedule_with_override(&mut conn, pid, &edited).unwrap();

        let s = crate::maintenance::dal::get_schedule(&conn, &sched_id)
            .unwrap()
            .unwrap();
        assert_eq!(s.task, "Edited task");
        assert_eq!(s.interval_months, 24);
        assert_eq!(s.notes, "edited notes");

        let status: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "applied");
    }

    #[test]
    fn approve_with_override_rejects_wrong_kind() {
        let (_d, mut conn) = fresh_conn();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_task",
                rationale: "r",
                diff_json: r#"{"title":"t","due_date":null}"#,
                skill: "test",
            },
        )
        .unwrap();
        let edited = crate::maintenance::MaintenanceScheduleDraft {
            asset_id: "x".into(),
            task: "x".into(),
            interval_months: 12,
            last_done_date: None,
            notes: String::new(),
        };
        let err = approve_add_maintenance_schedule_with_override(&mut conn, pid, &edited)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("not an add_maintenance_schedule"),
            "got: {}",
            err
        );
    }

    #[test]
    fn approve_with_override_rejects_non_pending() {
        let (_d, mut conn) = fresh_conn();
        let asset_id = insert_test_asset(&conn);
        let pid = insert_pending_schedule_proposal(&conn, &asset_id, "Service", 12, "att");
        reject(&conn, pid).unwrap();
        let edited = crate::maintenance::MaintenanceScheduleDraft {
            asset_id: asset_id.clone(),
            task: "x".into(),
            interval_months: 12,
            last_done_date: None,
            notes: String::new(),
        };
        let err = approve_add_maintenance_schedule_with_override(&mut conn, pid, &edited)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not pending"), "got: {}", err);
    }
}
