//! Tauri commands for PDF extraction (L4e).

use super::pipeline::{extract_and_propose, ExtractOutcome, TierRequest};
use crate::assistant::commands::Db;
use manor_core::assistant::proposal::{self, Proposal};
use manor_core::maintenance::MaintenanceScheduleDraft;
use tauri::{AppHandle, Manager, State};

fn attachments_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("attachments"))
}

#[tauri::command]
pub async fn pdf_extract_ollama(
    attachment_uuid: String,
    app: AppHandle,
    state: State<'_, Db>,
) -> Result<ExtractOutcome, String> {
    let db = state.0.clone();
    let dir = attachments_dir(&app)?;
    extract_and_propose(db, dir, attachment_uuid, TierRequest::Ollama)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pdf_extract_claude(
    attachment_uuid: String,
    app: AppHandle,
    state: State<'_, Db>,
) -> Result<ExtractOutcome, String> {
    let db = state.0.clone();
    let dir = attachments_dir(&app)?;
    extract_and_propose(db, dir, attachment_uuid, TierRequest::Claude)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pdf_extract_pending_proposals_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<Proposal>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, rationale, diff, status, proposed_at, applied_at, skill
             FROM proposal
             WHERE skill = 'pdf_extract'
               AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.asset_id') = ?1
             ORDER BY proposed_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&asset_id], |r| {
            Ok(Proposal {
                id: r.get(0)?,
                kind: r.get(1)?,
                rationale: r.get(2)?,
                diff: r.get(3)?,
                status: r.get(4)?,
                proposed_at: r.get(5)?,
                applied_at: r.get(6)?,
                skill: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
pub fn pdf_extract_pending_exists_for_attachment(
    attachment_uuid: String,
    state: State<'_, Db>,
) -> Result<bool, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM proposal
             WHERE skill = 'pdf_extract'
               AND kind = 'add_maintenance_schedule'
               AND status = 'pending'
               AND json_extract(diff, '$.source_attachment_uuid') = ?1",
            [&attachment_uuid],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

#[tauri::command]
pub fn pdf_extract_approve_as_is(
    proposal_id: i64,
    state: State<'_, Db>,
) -> Result<String, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::approve_add_maintenance_schedule(&mut conn, proposal_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pdf_extract_reject(proposal_id: i64, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::reject(&conn, proposal_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pdf_extract_approve_with_override(
    proposal_id: i64,
    draft: MaintenanceScheduleDraft,
    state: State<'_, Db>,
) -> Result<String, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    proposal::approve_add_maintenance_schedule_with_override(&mut conn, proposal_id, &draft)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::{
        db,
        proposal::{AddMaintenanceScheduleArgs, NewProposal},
    };
    use rusqlite::Connection;

    fn fresh_conn_with_asset() -> (tempfile::TempDir, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let asset = AssetDraft {
            name: "Boiler".into(),
            category: AssetCategory::Appliance,
            make: None,
            model: None,
            serial_number: None,
            purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, conn, asset_id)
    }

    fn insert_pending(conn: &Connection, asset_id: &str, task: &str, source: &str) -> i64 {
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.into(),
            task: task.into(),
            interval_months: 12,
            notes: String::new(),
            source_attachment_uuid: source.into(),
            tier: "ollama".into(),
        };
        let diff = serde_json::to_string(&args).unwrap();
        proposal::insert(
            conn,
            NewProposal {
                kind: "add_maintenance_schedule",
                rationale: "r",
                diff_json: &diff,
                skill: "pdf_extract",
            },
        )
        .unwrap()
    }

    #[test]
    fn pending_proposals_filters_by_asset() {
        let (_d, conn, asset_a) = fresh_conn_with_asset();
        let asset_b = asset_dal::insert_asset(
            &conn,
            &AssetDraft {
                name: "Other".into(),
                category: AssetCategory::Appliance,
                make: None,
                model: None,
                serial_number: None,
                purchase_date: None,
                notes: String::new(),
                hero_attachment_uuid: None,
            },
        )
        .unwrap();

        let _p1 = insert_pending(&conn, &asset_a, "A1", "uuid-A");
        let _p2 = insert_pending(&conn, &asset_a, "A2", "uuid-A");
        let _p3 = insert_pending(&conn, &asset_b, "B1", "uuid-B");

        let applied_pid = insert_pending(&conn, &asset_a, "A3", "uuid-A");
        conn.execute(
            "UPDATE proposal SET status = 'applied', applied_at = 1 WHERE id = ?1",
            rusqlite::params![applied_pid],
        )
        .unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT id FROM proposal
                 WHERE skill = 'pdf_extract' AND kind = 'add_maintenance_schedule'
                   AND status = 'pending'
                   AND json_extract(diff, '$.asset_id') = ?1",
            )
            .unwrap();
        let a_ids: Vec<i64> = stmt
            .query_map([&asset_a], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(a_ids.len(), 2);

        let b_ids: Vec<i64> = stmt
            .query_map([&asset_b], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(b_ids.len(), 1);
    }

    #[test]
    fn pending_exists_for_attachment_reflects_state() {
        let (_d, conn, asset_id) = fresh_conn_with_asset();
        let count_before: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proposal
                 WHERE skill = 'pdf_extract' AND kind = 'add_maintenance_schedule'
                   AND status = 'pending'
                   AND json_extract(diff, '$.source_attachment_uuid') = 'uuid-X'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_before, 0);

        insert_pending(&conn, &asset_id, "T", "uuid-X");
        let count_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proposal
                 WHERE skill = 'pdf_extract' AND kind = 'add_maintenance_schedule'
                   AND status = 'pending'
                   AND json_extract(diff, '$.source_attachment_uuid') = 'uuid-X'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_after, 1);
    }
}
