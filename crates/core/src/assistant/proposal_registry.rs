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

    let (kind, status, diff) = row
        .ok_or_else(|| ApplyError::Internal(format!("proposal {proposal_id} not found")))?;

    if status != Status::Pending.as_str() {
        return Err(ApplyError::Conflict("proposal not pending".into()));
    }

    let tx = conn
        .transaction()
        .map_err(|e| ApplyError::Internal(format!("tx: {e}")))?;

    let items_applied = match kind.as_str() {
        "add_task" => approvers::add_task::approve(&tx, proposal_id, &diff, None)?,
        "add_maintenance_schedule" => {
            approvers::add_maintenance_schedule::approve(&tx, proposal_id, &diff)?
        }
        unknown => return Err(ApplyError::UnknownKind(unknown.into())),
    };

    tx.commit()
        .map_err(|e| ApplyError::Internal(format!("tx commit: {e}")))?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied,
        items_failed: 0,
        errors: vec![],
    })
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
    use crate::assistant::proposal::{
        insert, AddMaintenanceScheduleArgs, NewProposal,
    };
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
                assert!(msg.contains("99999") && msg.contains("not found"), "got: {msg}")
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
            .query_row("SELECT COUNT(*) FROM task WHERE proposal_id = ?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(task_count, 0);
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
