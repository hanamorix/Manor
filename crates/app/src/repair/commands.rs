//! Tauri commands for repair-note + search (L4d).

use super::pipeline::{run_repair_search, PipelineOutcome, TierRequest};
use crate::assistant::commands::Db;
use manor_core::repair::RepairNote;
use tauri::State;

#[tauri::command]
pub async fn repair_search_ollama(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String> {
    let db = state.0.clone();
    run_repair_search(db, asset_id, symptom, TierRequest::Ollama)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn repair_search_claude(
    asset_id: String,
    symptom: String,
    state: State<'_, Db>,
) -> Result<PipelineOutcome, String> {
    let db = state.0.clone();
    run_repair_search(db, asset_id, symptom, TierRequest::Claude)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_list_for_asset(
    asset_id: String,
    state: State<'_, Db>,
) -> Result<Vec<RepairNote>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::list_for_asset(&conn, &asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_get(id: String, state: State<'_, Db>) -> Result<Option<RepairNote>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::get_repair_note(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn repair_note_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    manor_core::repair::dal::soft_delete_repair_note(&conn, &id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod integration_tests {
    use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use manor_core::assistant::db;
    use manor_core::repair::{dal as repair_dal, LlmTier, RepairNoteDraft, RepairSource};

    fn fresh_with_asset() -> (tempfile::TempDir, rusqlite::Connection, String) {
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

    #[test]
    fn list_and_delete_round_trip() {
        let (_d, conn, asset_id) = fresh_with_asset();
        let draft = RepairNoteDraft {
            asset_id: asset_id.clone(),
            symptom: "won't drain".into(),
            body_md: "check the filter".into(),
            sources: vec![RepairSource {
                url: "https://example.com".into(),
                title: "Example".into(),
            }],
            video_sources: None,
            tier: LlmTier::Ollama,
        };
        let id = repair_dal::insert_repair_note(&conn, &draft).unwrap();

        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);

        repair_dal::soft_delete_repair_note(&conn, &id).unwrap();
        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert!(rows.is_empty());

        let got = repair_dal::get_repair_note(&conn, &id).unwrap().unwrap();
        assert!(got.deleted_at.is_some());
    }

    #[test]
    fn list_orders_desc_and_excludes_trashed() {
        let (_d, conn, asset_id) = fresh_with_asset();
        let mk = |symptom: &str| RepairNoteDraft {
            asset_id: asset_id.clone(),
            symptom: symptom.into(),
            body_md: "body".into(),
            sources: vec![],
            video_sources: None,
            tier: LlmTier::Ollama,
        };
        let id1 = repair_dal::insert_repair_note(&conn, &mk("first")).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        repair_dal::insert_repair_note(&conn, &mk("second")).unwrap();

        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows[0].symptom, "second");
        assert_eq!(rows[1].symptom, "first");

        repair_dal::soft_delete_repair_note(&conn, &id1).unwrap();
        let rows = repair_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symptom, "second");
    }
}
