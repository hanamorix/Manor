//! Proposals — central AI-action artefacts.
//!
//! Phase 2 scaffolded the table + types. Phase 3a wires the first lifecycle:
//! `add_task` proposals can be applied (insert task + mark applied) or
//! rejected (mark rejected, no apply).

use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::assistant::proposal_error::ApplyError;
use crate::assistant::task;
use crate::assistant::tolerant;

pub mod approvers;

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
    pub fn as_str(self) -> &'static str {
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
    /// Raw JSON array of `ApplyError` values for partially-applied bundle
    /// proposals. `None` for single-item proposals or fully-applied bundles.
    /// Phase 1.G — first producer is Phase 2's bundled `add_chore`.
    pub apply_errors_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMaintenanceScheduleArgs {
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub notes: String,
    pub source_attachment_uuid: String,
    pub tier: String,
    /// Optional ISO-date the schedule was last performed. Populated when
    /// the user edits the proposal-edit drawer with a known last-done
    /// date; `None` for AI-generated proposals where it isn't known.
    /// Serde-default so legacy diffs (which omit the field entirely)
    /// continue to deserialise.
    #[serde(default)]
    pub last_done_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTaskArgs {
    pub title: String,
    /// Optional due date. qwen2.5 occasionally emits structured objects here
    /// instead of a plain string (e.g. `{year, month, day}`). We accept any
    /// JSON shape and coerce non-strings to `None` — the caller defaults to
    /// today when `None`, which is the right v0.1 behaviour anyway.
    #[serde(default, deserialize_with = "tolerant::iso_date")]
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompleteTaskArgs {
    #[serde(default, alias = "taskId")]
    pub task_id: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
}

fn default_chore_emoji() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddChoreItem {
    pub title: String,
    #[serde(default = "default_chore_emoji")]
    pub emoji: String,
    #[serde(deserialize_with = "tolerant::rrule_string")]
    pub rrule: String,
    #[serde(default)]
    pub first_due_ms: Option<i64>,
    #[serde(default)]
    pub rotation_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AddChoreArgs {
    Single(AddChoreItem),
    Bundle(Vec<AddChoreItem>),
}

impl AddChoreArgs {
    pub fn into_items(self) -> Vec<AddChoreItem> {
        match self {
            AddChoreArgs::Single(item) => vec![item],
            AddChoreArgs::Bundle(items) => items,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompleteChoreArgs {
    #[serde(default, alias = "choreId")]
    pub chore_id: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, alias = "completedBy")]
    pub completed_by: Option<i64>,
    #[serde(default, alias = "completedByName")]
    pub completed_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddTimeBlockArgs {
    pub title: String,
    #[serde(default = "default_time_block_kind")]
    pub kind: String,
    #[serde(alias = "dateMs")]
    pub date_ms: i64,
    #[serde(alias = "startTime")]
    pub start_time: String,
    #[serde(alias = "endTime")]
    pub end_time: String,
}

fn default_time_block_kind() -> String {
    "focus".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddRecurringBlockArgs {
    pub title: String,
    #[serde(default = "default_time_block_kind")]
    pub kind: String,
    #[serde(alias = "dateMs")]
    pub date_ms: i64,
    #[serde(alias = "startTime")]
    pub start_time: String,
    #[serde(alias = "endTime")]
    pub end_time: String,
    #[serde(deserialize_with = "tolerant::rrule_string")]
    pub rrule: String,
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
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill, apply_errors_json
             FROM proposal WHERE status = ?1 ORDER BY proposed_at",
            true,
        ),
        None => (
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill, apply_errors_json
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
            apply_errors_json: row.get("apply_errors_json")?,
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
///
/// **Phase 1.D shim.** The body delegates to `approvers::add_task::approve`,
/// which has the uniform signature the proposal_registry (Task 1.C) will use.
/// This wrapper retains kind/status validation, transaction lifecycle, and the
/// `Vec<Task>` return shape for existing call sites + tests. Phase 1.E
/// generalises this further; for now it stays.
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

    approvers::add_task::approve(&tx, id, &diff, Some(today_iso))
        .map_err(|e| anyhow::anyhow!("{e}"))?;

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
///
/// **Phase 1.D shim.** Delegates to `approvers::add_maintenance_schedule::approve_returning_id`,
/// then surfaces the inserted schedule's id for the legacy return contract.
/// Used by both the verbatim PDF flow and the override flow (after the
/// override flow has stamped the user's edits into the proposal's `diff`
/// via `proposal_registry::update_diff`).
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

    let schedule_id = approvers::add_maintenance_schedule::approve_returning_id(&tx, id, &diff)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    tx.commit()?;
    Ok(schedule_id)
}

/// Read the per-item `ApplyError` list persisted on a proposal row.
///
/// Returns `Ok(None)` when the column is `NULL` (single-item proposals,
/// fully-applied bundles, or pre-1.G rows). Returns `Ok(Some(vec))` after
/// deserialising the JSON array. Phase 1.G.
pub fn read_apply_errors(conn: &Connection, id: i64) -> Result<Option<Vec<ApplyError>>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT apply_errors_json FROM proposal WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("proposal {id} not found"))?;
    match raw {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

/// Persist the per-item `ApplyError` list on a proposal row as a JSON array.
///
/// Always overwrites; pass `&[]` to clear (writes a literal `[]`, not NULL).
/// Phase 1.G — first producer is Phase 2's bundled `add_chore`.
pub fn write_apply_errors(conn: &Connection, id: i64, errors: &[ApplyError]) -> Result<()> {
    let json = serde_json::to_string(errors)?;
    let n = conn.execute(
        "UPDATE proposal SET apply_errors_json = ?1 WHERE id = ?2",
        params![json, id],
    )?;
    if n == 0 {
        bail!("proposal {id} not found");
    }
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
            last_done_date: None,
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
        let pid =
            insert_pending_schedule_proposal(&conn, &asset_id, "Annual service", 12, "att-uuid-1");

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

    // (The bespoke approve_add_maintenance_schedule_with_override and its
    // tests were removed in Phase 1.E. Override semantics now flow through
    // proposal_registry::update_diff + approve_with_override; the override
    // test coverage lives in proposal_registry::tests.)

    // ── Task 1.G — apply_errors_json column + helpers ────────────────────

    #[test]
    fn v24_migration_adds_apply_errors_json_column() {
        let (_d, conn) = fresh_conn();
        // PRAGMA-driven probe: a SELECT of the new column on an empty table
        // succeeds iff the migration ran.
        conn.prepare("SELECT apply_errors_json FROM proposal LIMIT 0")
            .expect("apply_errors_json column should exist after db::init");
    }

    #[test]
    fn applied_proposal_has_null_apply_errors_by_default() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "No errors here");
        approve_add_task(&mut conn, pid, "2026-04-15").unwrap();

        // Column is NULL — round-tripping returns None.
        let errors = read_apply_errors(&conn, pid).unwrap();
        assert!(errors.is_none());

        // And the Proposal struct read back through `list` mirrors that.
        let row = list(&conn, None)
            .unwrap()
            .into_iter()
            .find(|p| p.id == pid)
            .unwrap();
        assert!(row.apply_errors_json.is_none());
    }

    #[test]
    fn write_then_read_apply_errors_round_trips() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Bundle");

        let errs = vec![
            ApplyError::StaleReference {
                entity: "asset".into(),
                id: "missing".into(),
            },
            ApplyError::InvalidArg {
                field: "interval_months".into(),
                reason: "must be positive".into(),
            },
            ApplyError::Conflict("row vanished".into()),
        ];
        write_apply_errors(&conn, pid, &errs).unwrap();

        let back = read_apply_errors(&conn, pid).unwrap().unwrap();
        assert_eq!(back.len(), 3);
        // Compare via Display since ApplyError isn't PartialEq.
        for (a, b) in errs.iter().zip(back.iter()) {
            assert_eq!(format!("{a}"), format!("{b}"));
        }

        // Proposal row exposes the raw JSON.
        let row = list(&conn, None)
            .unwrap()
            .into_iter()
            .find(|p| p.id == pid)
            .unwrap();
        let raw = row.apply_errors_json.expect("expected JSON, got NULL");
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 3);
    }

    #[test]
    fn write_apply_errors_with_empty_slice_writes_empty_array_not_null() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Empty");
        write_apply_errors(&conn, pid, &[]).unwrap();
        let back = read_apply_errors(&conn, pid).unwrap();
        let v = back.expect("empty slice persists as Some(vec![]), not None");
        assert!(v.is_empty());

        // Confirm the on-disk shape is `[]`, not NULL.
        let raw: Option<String> = conn
            .query_row(
                "SELECT apply_errors_json FROM proposal WHERE id = ?1",
                [pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(raw.as_deref(), Some("[]"));
    }

    #[test]
    fn read_apply_errors_unknown_id_errors() {
        let (_d, conn) = fresh_conn();
        let err = read_apply_errors(&conn, 99_999).unwrap_err().to_string();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn write_apply_errors_unknown_id_errors() {
        let (_d, conn) = fresh_conn();
        let err = write_apply_errors(&conn, 99_999, &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("not found"), "got: {err}");
    }
}
