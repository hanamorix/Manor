//! Central proposal-apply registry. Phase 1.C of v0.2 Hands.
//!
//! [`approve`] reads the proposal row, validates state, opens a transaction,
//! and dispatches to the per-kind approver in
//! [`crate::assistant::proposal::approvers`]. The approver returns a count of
//! items applied; we wrap it in an [`Applied`] for the IPC boundary.
//!
//! Two arms are wired today (`add_task`, `add_maintenance_schedule`); every
//! other kind falls through to [`ApplyError::UnknownKind`]. Each subsequent
//! phase of v0.2 Hands wires its own arm — no fall-through stubs.
//!
//! Override-mode (the proposal-edit Drawer path) lives in
//! `crate::assistant::proposal::approve_add_maintenance_schedule_with_override`
//! until Task 1.E generalises it into `approve_with_override`.

use rusqlite::{Connection, OptionalExtension};

use crate::assistant::proposal::{approvers, Status};
use crate::assistant::{Applied, ApplyError};

/// Approve a pending proposal by id. Dispatches to the per-kind approver.
///
/// Errors:
/// - [`ApplyError::Internal`] — proposal id not found, or any DB error.
/// - [`ApplyError::Conflict`] — proposal exists but is not in `pending` state.
/// - [`ApplyError::UnknownKind`] — kind is not yet wired into the registry.
/// - Anything the per-kind approver returns (e.g. [`ApplyError::InvalidArg`]
///   for malformed diff JSON).
///
/// On approver error the transaction drops uncommitted (auto-rollback).
pub fn approve(conn: &mut Connection, proposal_id: i64) -> Result<Applied, ApplyError> {
    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT kind, status, diff FROM proposal WHERE id = ?1",
            rusqlite::params![proposal_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| ApplyError::Internal(format!("db: {e}")))?;

    let (kind, status, diff) =
        row.ok_or_else(|| ApplyError::Internal(format!("proposal {proposal_id} not found")))?;

    if status != Status::Pending.as_str() {
        return Err(ApplyError::Conflict("proposal not pending".into()));
    }

    let tx = conn
        .transaction()
        .map_err(|e| ApplyError::Internal(format!("tx: {e}")))?;

    let applied = match kind.as_str() {
        "add_task" => {
            let items_applied = approvers::add_task::approve(&tx, proposal_id, &diff, None)?;
            Applied {
                proposal_id,
                status: Status::Applied,
                items_applied,
                items_failed: 0,
                errors: vec![],
            }
        }
        "add_chore" => approvers::add_chore::approve(&tx, proposal_id, &diff)?,
        "add_transaction" => approvers::add_ledger_transaction::approve(&tx, proposal_id, &diff)?,
        "complete_chore" => approvers::complete_chore::approve(&tx, proposal_id, &diff)?,
        "complete_task" => approvers::complete_task::approve(&tx, proposal_id, &diff)?,
        "add_time_block" => approvers::add_time_block::approve(&tx, proposal_id, &diff)?,
        "add_recurring_block" => approvers::add_recurring_block::approve(&tx, proposal_id, &diff)?,
        "add_maintenance_schedule" => {
            let items_applied =
                approvers::add_maintenance_schedule::approve(&tx, proposal_id, &diff)?;
            Applied {
                proposal_id,
                status: Status::Applied,
                items_applied,
                items_failed: 0,
                errors: vec![],
            }
        }
        unknown => return Err(ApplyError::UnknownKind(unknown.into())),
    };

    tx.commit()
        .map_err(|e| ApplyError::Internal(format!("tx commit: {e}")))?;

    Ok(applied)
}

/// Overwrite the `diff` column of a *pending* proposal.
///
/// Used by [`approve_with_override`] to inject the user-edited diff before
/// dispatching to the same per-kind approver as the verbatim path. Does NOT
/// validate the JSON against any kind-specific schema — that's the
/// approver's job.
///
/// Errors:
/// - [`ApplyError::Conflict`] — proposal exists but is not `pending`.
/// - [`ApplyError::Internal`] — proposal id not found, or any DB error.
pub fn update_diff(conn: &Connection, id: i64, edited_diff_json: &str) -> Result<(), ApplyError> {
    let rows = conn
        .execute(
            "UPDATE proposal SET diff = ?1 WHERE id = ?2 AND status = 'pending'",
            rusqlite::params![edited_diff_json, id],
        )
        .map_err(|e| ApplyError::Internal(format!("db update_diff: {e}")))?;
    if rows == 0 {
        // Either id doesn't exist, or status != pending. Distinguish:
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM proposal WHERE id = ?1",
                rusqlite::params![id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| ApplyError::Internal(format!("db: {e}")))?
            .unwrap_or(false);
        return Err(if exists {
            ApplyError::Conflict("proposal not pending".into())
        } else {
            ApplyError::Internal(format!("proposal {id} not found"))
        });
    }
    Ok(())
}

/// Approve a pending proposal whose diff has been edited by the user.
///
/// Equivalent to [`update_diff`] followed by [`approve`] — the per-kind
/// approver runs on the edited diff, so the same validation, transactional
/// guarantees, and error shapes apply.
///
/// `update_diff` is **not** transactional with the subsequent `approve` —
/// if the approver returns an error, the diff change is already committed
/// (the proposal stays `pending` with the edited diff persisted, ready for
/// the user to retry or reject). This matches the expected drawer-edit UX:
/// edits are visible even when approval fails.
pub fn approve_with_override(
    conn: &mut Connection,
    proposal_id: i64,
    edited_diff_json: &str,
) -> Result<Applied, ApplyError> {
    update_diff(conn, proposal_id, edited_diff_json)?;
    approve(conn, proposal_id)
}

/// Read the `kind` column for a proposal. Exposed for the Tauri layer to
/// switch on if needed (e.g. routing the override-mode call before invoking
/// the registry).
pub fn read_kind(conn: &Connection, proposal_id: i64) -> Result<String, ApplyError> {
    conn.query_row(
        "SELECT kind FROM proposal WHERE id = ?1",
        rusqlite::params![proposal_id],
        |r| r.get::<_, String>(0),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            ApplyError::Internal(format!("proposal {proposal_id} not found"))
        }
        other => ApplyError::Internal(format!("db: {other}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use crate::assistant::proposal::{insert, AddMaintenanceScheduleArgs, NewProposal};
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

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

    fn make_add_task_proposal(conn: &Connection, title: &str) -> i64 {
        let diff = serde_json::json!({ "title": title }).to_string();
        insert(
            conn,
            NewProposal {
                kind: "add_task",
                rationale: "registry-test",
                diff_json: &diff,
                skill: "tasks",
            },
        )
        .unwrap()
    }

    fn make_add_maint_proposal(conn: &Connection, asset_id: &str) -> i64 {
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.into(),
            task: "Annual service".into(),
            interval_months: 12,
            notes: String::new(),
            source_attachment_uuid: "att-1".into(),
            tier: "ollama".into(),
            last_done_date: None,
        };
        let diff = serde_json::to_string(&args).unwrap();
        insert(
            conn,
            NewProposal {
                kind: "add_maintenance_schedule",
                rationale: "registry-test",
                diff_json: &diff,
                skill: "pdf_extract",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_dispatches_add_task_proposal() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Pick up prescription");

        let applied = approve(&mut conn, pid).unwrap();
        assert_eq!(applied.proposal_id, pid);
        assert_eq!(applied.status, Status::Applied);
        assert_eq!(applied.items_applied, 1);
        assert_eq!(applied.items_failed, 0);
        assert!(applied.errors.is_empty());

        // Task landed in DB tied to this proposal.
        let title: String = conn
            .query_row(
                "SELECT title FROM task WHERE proposal_id = ?1",
                [pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Pick up prescription");

        // Proposal flipped to applied.
        let status: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "applied");
    }

    #[test]
    fn approve_dispatches_add_maintenance_schedule_proposal() {
        let (_d, mut conn) = fresh_conn();
        let asset_id = insert_test_asset(&conn);
        let pid = make_add_maint_proposal(&conn, &asset_id);

        let applied = approve(&mut conn, pid).unwrap();
        assert_eq!(applied.proposal_id, pid);
        assert_eq!(applied.status, Status::Applied);
        assert_eq!(applied.items_applied, 1);
        assert_eq!(applied.items_failed, 0);
        assert!(applied.errors.is_empty());

        // Schedule was inserted under the asset.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM maintenance_schedule WHERE asset_id = ?1",
                [&asset_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let status: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "applied");
    }

    #[test]
    fn approve_unknown_kind_returns_unknown_kind_error() {
        let (_d, mut conn) = fresh_conn();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "foo",
                rationale: "registry-test",
                diff_json: "{}",
                skill: "test",
            },
        )
        .unwrap();

        let err = approve(&mut conn, pid).unwrap_err();
        match err {
            ApplyError::UnknownKind(k) => assert_eq!(k, "foo"),
            other => panic!("expected UnknownKind, got {other:?}"),
        }

        // Proposal stays pending — no partial state.
        let status: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn approve_proposal_not_pending_returns_conflict() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "X");
        // First approve flips it to applied.
        approve(&mut conn, pid).unwrap();

        let err = approve(&mut conn, pid).unwrap_err();
        match err {
            ApplyError::Conflict(msg) => assert_eq!(msg, "proposal not pending"),
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn approve_proposal_not_found_returns_internal() {
        let (_d, mut conn) = fresh_conn();
        let err = approve(&mut conn, 99_999).unwrap_err();
        match err {
            ApplyError::Internal(msg) => {
                assert!(
                    msg.contains("99999") && msg.contains("not found"),
                    "got: {msg}"
                )
            }
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn approve_invalid_diff_propagates_invalid_arg() {
        let (_d, mut conn) = fresh_conn();
        // Insert an add_task proposal whose diff is not valid JSON for AddTaskArgs.
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_task",
                rationale: "registry-test",
                diff_json: "not json",
                skill: "tasks",
            },
        )
        .unwrap();

        let err = approve(&mut conn, pid).unwrap_err();
        match err {
            ApplyError::InvalidArg { field, .. } => assert_eq!(field, "diff"),
            other => panic!("expected InvalidArg, got {other:?}"),
        }

        // Transaction rolled back — proposal still pending, no task inserted.
        let status: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "pending");

        let task_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task WHERE proposal_id = ?1",
                [pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(task_count, 0);
    }

    // ── Task 1.E — generic approve_with_override ─────────────────────────

    #[test]
    fn approve_with_override_persists_edited_diff_for_add_task() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Original title");

        let edited = serde_json::json!({ "title": "Edited title" }).to_string();
        let applied = approve_with_override(&mut conn, pid, &edited).unwrap();
        assert_eq!(applied.proposal_id, pid);
        assert_eq!(applied.status, Status::Applied);
        assert_eq!(applied.items_applied, 1);

        // The persisted task carries the *edited* title, not the original.
        let title: String = conn
            .query_row(
                "SELECT title FROM task WHERE proposal_id = ?1",
                [pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Edited title");

        // The proposal row's diff column now holds the edited JSON.
        let diff_on_row: String = conn
            .query_row("SELECT diff FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(diff_on_row, edited);
    }

    #[test]
    fn approve_with_override_for_add_maintenance_schedule_uses_edited_diff() {
        let (_d, mut conn) = fresh_conn();
        let asset_id = insert_test_asset(&conn);
        let pid = make_add_maint_proposal(&conn, &asset_id);

        let edited = serde_json::to_string(&AddMaintenanceScheduleArgs {
            asset_id: asset_id.clone(),
            task: "Edited service".into(),
            interval_months: 24,
            notes: "Edited notes".into(),
            source_attachment_uuid: "att-1".into(),
            tier: "ollama".into(),
            last_done_date: None,
        })
        .unwrap();

        let applied = approve_with_override(&mut conn, pid, &edited).unwrap();
        assert_eq!(applied.status, Status::Applied);
        assert_eq!(applied.items_applied, 1);

        let (task, interval): (String, i32) = conn
            .query_row(
                "SELECT task, interval_months FROM maintenance_schedule WHERE asset_id = ?1",
                [&asset_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(task, "Edited service");
        assert_eq!(interval, 24);
    }

    #[test]
    fn approve_with_override_returns_conflict_for_non_pending() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "X");
        // First approval flips it to applied.
        approve(&mut conn, pid).unwrap();

        let edited = serde_json::json!({ "title": "Y" }).to_string();
        let err = approve_with_override(&mut conn, pid, &edited).unwrap_err();
        match err {
            ApplyError::Conflict(msg) => assert_eq!(msg, "proposal not pending"),
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn update_diff_returns_internal_for_unknown_id() {
        let (_d, conn) = fresh_conn();
        let err = update_diff(&conn, 99_999, r#"{"title":"x"}"#).unwrap_err();
        match err {
            ApplyError::Internal(msg) => {
                assert!(
                    msg.contains("99999") && msg.contains("not found"),
                    "got: {msg}"
                )
            }
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn update_diff_returns_conflict_for_non_pending() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "X");
        approve(&mut conn, pid).unwrap();

        let err = update_diff(&conn, pid, r#"{"title":"Y"}"#).unwrap_err();
        match err {
            ApplyError::Conflict(msg) => assert_eq!(msg, "proposal not pending"),
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn update_diff_persists_for_pending_proposal() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Original");
        update_diff(&conn, pid, r#"{"title":"Edited"}"#).unwrap();
        let diff: String = conn
            .query_row("SELECT diff FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(diff, r#"{"title":"Edited"}"#);
    }

    #[test]
    fn approve_with_override_propagates_approver_invalid_arg() {
        let (_d, mut conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "Real");

        // The approver will reject "not json" with InvalidArg{field:"diff"}.
        let err = approve_with_override(&mut conn, pid, "not json").unwrap_err();
        match err {
            ApplyError::InvalidArg { field, .. } => assert_eq!(field, "diff"),
            other => panic!("expected InvalidArg, got {other:?}"),
        }

        // Approver transaction rolled back — proposal still pending, no task.
        // The diff *was* updated though (update_diff is not in the approve tx).
        let status: String = conn
            .query_row("SELECT status FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "pending");

        let task_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task WHERE proposal_id = ?1",
                [pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(task_count, 0);

        let diff: String = conn
            .query_row("SELECT diff FROM proposal WHERE id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(diff, "not json");
    }

    #[test]
    fn read_kind_returns_kind_for_existing_proposal() {
        let (_d, conn) = fresh_conn();
        let pid = make_add_task_proposal(&conn, "X");
        let kind = read_kind(&conn, pid).unwrap();
        assert_eq!(kind, "add_task");
    }

    #[test]
    fn read_kind_unknown_id_returns_internal() {
        let (_d, conn) = fresh_conn();
        let err = read_kind(&conn, 99_999).unwrap_err();
        match err {
            ApplyError::Internal(msg) => assert!(msg.contains("not found"), "got: {msg}"),
            other => panic!("expected Internal, got {other:?}"),
        }
    }
}
