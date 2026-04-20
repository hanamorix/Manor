//! Repair-note DAL (L4d).

use super::{LlmTier, RepairNote, RepairNoteDraft, RepairSource};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn insert_repair_note(conn: &Connection, draft: &RepairNoteDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let sources_json = serde_json::to_string(&draft.sources)?;
    let video_sources_json = draft
        .video_sources
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    conn.execute(
        "INSERT INTO repair_note
           (id, asset_id, symptom, body_md, sources, video_sources, tier,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id,
            draft.asset_id,
            draft.symptom,
            draft.body_md,
            sources_json,
            video_sources_json,
            draft.tier.as_str(),
            now,
        ],
    )?;
    Ok(id)
}

pub fn get_repair_note(conn: &Connection, id: &str) -> Result<Option<RepairNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, symptom, body_md, sources, video_sources, tier,
                created_at, updated_at, deleted_at
         FROM repair_note WHERE id = ?1",
    )?;
    stmt.query_row(params![id], row_to_repair_note)
        .optional()
        .map_err(Into::into)
}

pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<RepairNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, symptom, body_md, sources, video_sources, tier,
                created_at, updated_at, deleted_at
         FROM repair_note
         WHERE asset_id = ?1 AND deleted_at IS NULL
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![asset_id], row_to_repair_note)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn soft_delete_repair_note(conn: &Connection, id: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE repair_note SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now_secs(), id],
    )?;
    if changed == 0 {
        return Err(anyhow!("Repair note not found or already deleted"));
    }
    Ok(())
}

pub fn restore_repair_note(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE repair_note SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn permanent_delete_repair_note(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM repair_note WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}

fn row_to_repair_note(row: &Row) -> rusqlite::Result<RepairNote> {
    let sources_json: String = row.get("sources")?;
    let sources: Vec<RepairSource> = serde_json::from_str(&sources_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    let video_sources_json: Option<String> = row.get("video_sources")?;
    let video_sources: Option<Vec<RepairSource>> = match video_sources_json {
        Some(s) => Some(serde_json::from_str(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                )),
            )
        })?),
        None => None,
    };
    let tier_str: String = row.get("tier")?;
    let tier = LlmTier::parse(&tier_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    Ok(RepairNote {
        id: row.get("id")?,
        asset_id: row.get("asset_id")?,
        symptom: row.get("symptom")?,
        body_md: row.get("body_md")?,
        sources,
        video_sources,
        tier,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection, String) {
        let dir = tempdir().unwrap();
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

    fn draft(asset_id: &str) -> RepairNoteDraft {
        RepairNoteDraft {
            asset_id: asset_id.into(),
            symptom: "won't drain".into(),
            body_md: "Check the filter first. If still clogged, remove the drain hose.".into(),
            sources: vec![
                RepairSource {
                    url: "https://example.com/a".into(),
                    title: "A".into(),
                },
                RepairSource {
                    url: "https://example.com/b".into(),
                    title: "B".into(),
                },
            ],
            video_sources: None,
            tier: LlmTier::Ollama,
        }
    }

    #[test]
    fn insert_and_get_round_trip_with_video_none() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        assert_eq!(got.asset_id, asset_id);
        assert_eq!(got.symptom, "won't drain");
        assert_eq!(got.sources.len(), 2);
        assert_eq!(got.sources[0].url, "https://example.com/a");
        assert!(got.video_sources.is_none());
        assert_eq!(got.tier, LlmTier::Ollama);
    }

    #[test]
    fn insert_round_trip_with_video_sources() {
        let (_d, conn, asset_id) = fresh();
        let mut d = draft(&asset_id);
        d.video_sources = Some(vec![RepairSource {
            url: "https://www.youtube.com/watch?v=abc".into(),
            title: "Fix Your Boiler".into(),
        }]);
        let id = insert_repair_note(&conn, &d).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        let vs = got.video_sources.unwrap();
        assert_eq!(vs.len(), 1);
        assert_eq!(vs[0].title, "Fix Your Boiler");
    }

    #[test]
    fn list_for_asset_orders_desc_and_excludes_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id1 = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut d2 = draft(&asset_id);
        d2.symptom = "second".into();
        let _id2 = insert_repair_note(&conn, &d2).unwrap();

        let rows = list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symptom, "second");

        soft_delete_repair_note(&conn, &id1).unwrap();
        let rows = list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symptom, "second");
    }

    #[test]
    fn soft_delete_restore_round_trip() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        soft_delete_repair_note(&conn, &id).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        assert!(got.deleted_at.is_some());
        restore_repair_note(&conn, &id).unwrap();
        let got = get_repair_note(&conn, &id).unwrap().unwrap();
        assert!(got.deleted_at.is_none());
    }

    #[test]
    fn permanent_delete_only_removes_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        permanent_delete_repair_note(&conn, &id).unwrap();
        assert!(get_repair_note(&conn, &id).unwrap().is_some());
        soft_delete_repair_note(&conn, &id).unwrap();
        permanent_delete_repair_note(&conn, &id).unwrap();
        assert!(get_repair_note(&conn, &id).unwrap().is_none());
    }

    #[test]
    fn soft_delete_returns_error_when_already_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_repair_note(&conn, &draft(&asset_id)).unwrap();
        soft_delete_repair_note(&conn, &id).unwrap();
        let err = soft_delete_repair_note(&conn, &id).unwrap_err().to_string();
        assert!(err.contains("not found or already deleted"), "got: {}", err);
    }
}
