//! Approver for `add_maintenance_schedule` proposals.
//!
//! Phase 1.D — extracted from the monolithic `proposal.rs`. Mirrors the shape
//! of `add_task::approve`. Override-mode (`approve_*_with_override`) stays
//! in `proposal/mod.rs` for now and gets generalised in Task 1.E.

use rusqlite::{params, Transaction};

use crate::assistant::proposal::AddMaintenanceScheduleArgs;
use crate::assistant::proposal_error::ApplyError;
use crate::maintenance::{self, MaintenanceScheduleDraft};

/// Approve a single `add_maintenance_schedule` proposal verbatim (no override).
///
/// Pre-conditions (caller checks): proposal exists, kind matches, status is
/// `pending`. Inserts the schedule via `maintenance::dal::insert_schedule`,
/// marks the proposal applied, returns `Ok(1)` for one item applied.
pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<usize, ApplyError> {
    approve_returning_id(tx, proposal_id, diff).map(|_| 1)
}

/// Same as [`approve`] but additionally surfaces the freshly-inserted
/// `maintenance_schedule.id`. Phase 1.D shim in `proposal/mod.rs` needs this
/// to preserve its legacy `Result<String>` return contract; the proposal
/// registry path (Task 1.C) only wants the count and uses [`approve`].
pub fn approve_returning_id(
    tx: &Transaction,
    proposal_id: i64,
    diff: &str,
) -> Result<String, ApplyError> {
    let args: AddMaintenanceScheduleArgs =
        serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
            field: "diff".into(),
            reason: e.to_string(),
        })?;

    let draft = MaintenanceScheduleDraft {
        asset_id: args.asset_id,
        task: args.task,
        interval_months: args.interval_months,
        last_done_date: None,
        notes: args.notes,
    };

    let schedule_id = maintenance::dal::insert_schedule(tx, &draft)
        .map_err(|e| ApplyError::Internal(format!("insert_schedule failed: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    tx.execute(
        "UPDATE proposal SET status = 'applied', applied_at = ?1 WHERE id = ?2",
        params![now, proposal_id],
    )
    .map_err(|e| ApplyError::Internal(format!("proposal update failed: {e}")))?;

    Ok(schedule_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use crate::assistant::db;
    use crate::assistant::proposal::{insert, NewProposal};
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn insert_test_asset(conn: &Connection) -> String {
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

    #[test]
    fn approve_inserts_schedule_and_marks_applied() {
        let (_d, mut conn) = fresh_conn();
        let asset_id = insert_test_asset(&conn);

        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.clone(),
            task: "Annual service".into(),
            interval_months: 12,
            notes: "from manual".into(),
            source_attachment_uuid: "att-1".into(),
            tier: "ollama".into(),
        };
        let diff = serde_json::to_string(&args).unwrap();
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_maintenance_schedule",
                rationale: "module-test",
                diff_json: &diff,
                skill: "pdf_extract",
            },
        )
        .unwrap();

        let tx = conn.transaction().unwrap();
        let n = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(n, 1);

        // The schedule was created against this asset.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM maintenance_schedule WHERE asset_id = ?1",
                [&asset_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

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
    fn approve_invalid_diff_returns_invalid_arg() {
        let (_d, mut conn) = fresh_conn();
        // Insert a placeholder proposal so the row exists.
        let pid = insert(
            &conn,
            NewProposal {
                kind: "add_maintenance_schedule",
                rationale: "r",
                diff_json: r#"{"asset_id":"a","task":"t","interval_months":12,"notes":"","source_attachment_uuid":"","tier":"ollama"}"#,
                skill: "pdf_extract",
            },
        )
        .unwrap();

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, "not json").unwrap_err();
        match err {
            ApplyError::InvalidArg { field, .. } => assert_eq!(field, "diff"),
            other => panic!("expected InvalidArg, got {other:?}"),
        }
    }
}
