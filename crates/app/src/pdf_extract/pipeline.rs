//! PDF extraction pipeline orchestrator (L4e).

use super::claude_client::ClaudeExtractClient;
use super::ollama_client::OllamaExtractClient;
use anyhow::{anyhow, Result};
use manor_core::assistant::proposal::{self, AddMaintenanceScheduleArgs, NewProposal};
use manor_core::pdf_extract::{
    llm::extract_schedules_via_llm,
    text::{cap_for_tier, extract_text_from_pdf},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierRequest {
    Ollama,
    Claude,
}

impl TierRequest {
    fn is_claude(self) -> bool {
        matches!(self, TierRequest::Claude)
    }
    fn as_str(self) -> &'static str {
        match self {
            TierRequest::Ollama => "ollama",
            TierRequest::Claude => "claude",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractOutcome {
    pub proposals_inserted: i64,
    pub replaced_pending_count: i64,
}

pub async fn extract_and_propose(
    db: Arc<Mutex<rusqlite::Connection>>,
    attachments_dir: PathBuf,
    attachment_uuid: String,
    tier: TierRequest,
) -> Result<ExtractOutcome> {
    // 1. Resolve attachment → (asset_id, asset_name).
    let (asset_id, asset_name) = {
        let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
        let row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT entity_type, entity_id FROM attachment
                 WHERE uuid = ?1 AND deleted_at IS NULL",
                [&attachment_uuid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let (entity_type, entity_id) = row.ok_or_else(|| anyhow!("Attachment not found"))?;
        if entity_type.as_deref() != Some("asset") {
            return Err(anyhow!("Attachment not linked to an asset"));
        }
        let asset_id = entity_id.ok_or_else(|| anyhow!("Attachment not linked to an asset"))?;
        let asset_name: Option<String> = conn
            .query_row(
                "SELECT name FROM asset WHERE id = ?1 AND deleted_at IS NULL",
                [&asset_id],
                |r| r.get(0),
            )
            .ok();
        let asset_name = asset_name.ok_or_else(|| anyhow!("Asset not found"))?;
        (asset_id, asset_name)
    };

    // 2. Build PDF path (attachments stored without extension per L4a convention).
    let path = manor_core::attachment::file_path(&attachments_dir, &attachment_uuid);

    // 3. Extract text. ExtractError bubbles via Display.
    let text = extract_text_from_pdf(&path).map_err(|e| anyhow!(e.to_string()))?;

    // 4. Cap for tier.
    let capped = cap_for_tier(&text, tier.is_claude());

    // 5. Build LlmClient + invoke.
    let (schedules, remote_call_log_id) = match tier {
        TierRequest::Ollama => {
            let model = {
                let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;
                crate::assistant::ollama::resolve_model(&conn)
            };
            let client = OllamaExtractClient::new(model);
            let schedules = extract_schedules_via_llm(&capped, &client).await?;
            (schedules, None)
        }
        TierRequest::Claude => {
            let log_sink = Arc::new(Mutex::new(None));
            let client = ClaudeExtractClient {
                db: db.clone(),
                asset_name: asset_name.clone(),
                remote_call_log_id_sink: log_sink.clone(),
            };
            let schedules = extract_schedules_via_llm(&capped, &client).await?;
            let captured = *log_sink.lock().unwrap();
            (schedules, captured)
        }
    };

    // 6. Replace pending + insert new (test-seam entry point).
    run_pipeline_persist(
        db,
        asset_id,
        attachment_uuid,
        schedules,
        tier,
        remote_call_log_id,
    )
}

pub(crate) fn run_pipeline_persist(
    db: Arc<Mutex<rusqlite::Connection>>,
    asset_id: String,
    attachment_uuid: String,
    schedules: Vec<manor_core::pdf_extract::ExtractedSchedule>,
    tier: TierRequest,
    remote_call_log_id: Option<i64>,
) -> Result<ExtractOutcome> {
    let conn = db.lock().map_err(|e| anyhow!("db lock: {e}"))?;

    // Replace pending proposals from the same source attachment.
    let replaced = conn.execute(
        "UPDATE proposal SET status = 'rejected'
         WHERE skill = 'pdf_extract'
           AND kind = 'add_maintenance_schedule'
           AND status = 'pending'
           AND json_extract(diff, '$.source_attachment_uuid') = ?1",
        [&attachment_uuid],
    )? as i64;

    // Insert each schedule as a proposal.
    let mut inserted = 0i64;
    for sched in schedules {
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.clone(),
            task: sched.task,
            interval_months: sched.interval_months,
            notes: sched.notes,
            source_attachment_uuid: attachment_uuid.clone(),
            tier: tier.as_str().to_string(),
        };
        let diff_json = serde_json::to_string(&args)?;
        let pid = proposal::insert(
            &conn,
            NewProposal {
                kind: "add_maintenance_schedule",
                rationale: &sched.rationale,
                diff_json: &diff_json,
                skill: "pdf_extract",
            },
        )?;
        if let Some(log_id) = remote_call_log_id {
            conn.execute(
                "UPDATE proposal SET remote_call_log_id = ?1 WHERE id = ?2",
                rusqlite::params![log_id, pid],
            )?;
        }
        inserted += 1;
    }

    Ok(ExtractOutcome {
        proposals_inserted: inserted,
        replaced_pending_count: replaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::db;
    use manor_core::pdf_extract::ExtractedSchedule;

    fn fresh_db() -> (tempfile::TempDir, Arc<Mutex<rusqlite::Connection>>, String) {
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
        (dir, Arc::new(Mutex::new(conn)), asset_id)
    }

    fn insert_pending_schedule_proposal(
        db: &Arc<Mutex<rusqlite::Connection>>,
        asset_id: &str,
        source_attachment_uuid: &str,
        task: &str,
    ) -> i64 {
        let conn = db.lock().unwrap();
        let args = AddMaintenanceScheduleArgs {
            asset_id: asset_id.into(),
            task: task.into(),
            interval_months: 12,
            notes: String::new(),
            source_attachment_uuid: source_attachment_uuid.into(),
            tier: "ollama".into(),
        };
        let diff_json = serde_json::to_string(&args).unwrap();
        proposal::insert(
            &conn,
            NewProposal {
                kind: "add_maintenance_schedule",
                rationale: "old",
                diff_json: &diff_json,
                skill: "pdf_extract",
            },
        )
        .unwrap()
    }

    fn sample_extract(n: usize) -> Vec<ExtractedSchedule> {
        (0..n)
            .map(|i| ExtractedSchedule {
                task: format!("Task {}", i),
                interval_months: 12,
                notes: "".into(),
                rationale: format!("Rationale {}", i),
            })
            .collect()
    }

    #[test]
    fn pipeline_persist_replaces_pending_from_same_attachment() {
        let (_d, db, asset_id) = fresh_db();
        let _p1 = insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "old1");
        let _p2 = insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "old2");

        let out = run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-A".into(),
            sample_extract(1),
            TierRequest::Ollama,
            None,
        )
        .unwrap();
        assert_eq!(out.replaced_pending_count, 2);
        assert_eq!(out.proposals_inserted, 1);

        let conn = db.lock().unwrap();
        let rejected_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proposal WHERE status = 'rejected'
                 AND kind = 'add_maintenance_schedule' AND skill = 'pdf_extract'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rejected_count, 2);
        let pending_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proposal WHERE status = 'pending'
                 AND kind = 'add_maintenance_schedule' AND skill = 'pdf_extract'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pending_count, 1);
    }

    #[test]
    fn pipeline_persist_does_not_touch_other_attachments() {
        let (_d, db, asset_id) = fresh_db();
        let _p_b = insert_pending_schedule_proposal(&db, &asset_id, "uuid-B", "B-pending");

        run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-A".into(),
            sample_extract(1),
            TierRequest::Ollama,
            None,
        )
        .unwrap();

        let conn = db.lock().unwrap();
        let b_still_pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proposal WHERE status = 'pending'
                 AND json_extract(diff, '$.source_attachment_uuid') = 'uuid-B'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(b_still_pending, 1);
    }

    #[test]
    fn pipeline_persist_does_not_touch_applied_or_rejected_same_attachment() {
        let (_d, db, asset_id) = fresh_db();
        let applied_pid = insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "applied-one");
        let rejected_pid =
            insert_pending_schedule_proposal(&db, &asset_id, "uuid-A", "rejected-one");
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "UPDATE proposal SET status = 'applied', applied_at = 1 WHERE id = ?1",
                rusqlite::params![applied_pid],
            )
            .unwrap();
            conn.execute(
                "UPDATE proposal SET status = 'rejected' WHERE id = ?1",
                rusqlite::params![rejected_pid],
            )
            .unwrap();
        }

        let out = run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-A".into(),
            sample_extract(1),
            TierRequest::Ollama,
            None,
        )
        .unwrap();
        assert_eq!(out.replaced_pending_count, 0);

        let conn = db.lock().unwrap();
        let applied_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proposal WHERE status = 'applied' AND id = ?1",
                rusqlite::params![applied_pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(applied_count, 1);
        let rejected_row_status: String = conn
            .query_row(
                "SELECT status FROM proposal WHERE id = ?1",
                rusqlite::params![rejected_pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rejected_row_status, "rejected");
    }

    #[test]
    fn pipeline_persist_inserts_correct_diff_shape() {
        let (_d, db, asset_id) = fresh_db();
        run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-X".into(),
            sample_extract(2),
            TierRequest::Ollama,
            None,
        )
        .unwrap();

        let conn = db.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT kind, skill, json_extract(diff, '$.task'),
                        json_extract(diff, '$.source_attachment_uuid'),
                        json_extract(diff, '$.tier')
                 FROM proposal WHERE status = 'pending' ORDER BY id ASC",
            )
            .unwrap();
        let rows: Vec<(String, String, String, String, String)> = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert_eq!(row.0, "add_maintenance_schedule");
            assert_eq!(row.1, "pdf_extract");
            assert_eq!(row.3, "uuid-X");
            assert_eq!(row.4, "ollama");
        }
        assert_eq!(rows[0].2, "Task 0");
        assert_eq!(rows[1].2, "Task 1");
    }

    #[test]
    fn pipeline_persist_zero_inserted_on_empty_extraction() {
        let (_d, db, asset_id) = fresh_db();
        let out = run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-Z".into(),
            vec![],
            TierRequest::Ollama,
            None,
        )
        .unwrap();
        assert_eq!(out.proposals_inserted, 0);
    }

    #[test]
    fn pipeline_persist_captures_remote_call_log_id_for_claude() {
        let (_d, db, asset_id) = fresh_db();
        run_pipeline_persist(
            db.clone(),
            asset_id.clone(),
            "uuid-claude".into(),
            sample_extract(1),
            TierRequest::Claude,
            Some(42),
        )
        .unwrap();

        let conn = db.lock().unwrap();
        let stored_log_id: Option<i64> = conn
            .query_row(
                "SELECT remote_call_log_id FROM proposal WHERE kind = 'add_maintenance_schedule'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored_log_id, Some(42));
    }
}
