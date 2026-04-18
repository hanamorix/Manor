//! Asset attachment staging: copy local file into attachments dir, create attachment row,
//! link to the asset (by UUID text entity_id). Orphan sweep at startup reaps crash-leftover
//! hero stagings (entity_id IS NULL, entity_type='asset', >24h old).

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

const ORPHAN_AGE_SECS: i64 = 24 * 60 * 60;

/// Copy a file at `source_path` into the attachments root, create an attachment row with
/// entity_type='asset' and entity_id linked to the asset (text UUID), and return the new
/// attachment uuid. For hero: caller follows up with `asset::dal::set_hero_attachment`.
pub fn attach_file(
    conn: &Connection,
    attachments_dir: &Path,
    source_path: &Path,
    asset_id: &str,
) -> Result<String> {
    let bytes = std::fs::read(source_path)
        .with_context(|| format!("reading {}", source_path.display()))?;
    let original_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let mime = guess_mime(&original_name);

    let att = manor_core::attachment::store(
        conn,
        attachments_dir,
        &bytes,
        &original_name,
        &mime,
        Some("asset"),
        None, // staged; linked below
    )?;
    manor_core::attachment::link_to_entity(conn, att.id, "asset", asset_id)?;
    Ok(att.uuid)
}

fn guess_mime(filename: &str) -> String {
    let lower = filename.to_lowercase();
    if lower.ends_with(".pdf") {
        "application/pdf".into()
    } else if lower.ends_with(".png") {
        "image/png".into()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".into()
    } else if lower.ends_with(".webp") {
        "image/webp".into()
    } else if lower.ends_with(".heic") {
        "image/heic".into()
    } else {
        "application/octet-stream".into()
    }
}

/// Sweep crash-orphaned asset stagings (entity_type='asset', entity_id IS NULL, age >24h).
/// Mirrors `recipe::stage_sweep::run_on_startup` — uses seconds for `created_at` comparison.
pub fn stage_sweep_run_on_startup(conn: &Connection, attachments_dir: &Path) -> Result<usize> {
    let cutoff = chrono::Utc::now().timestamp() - ORPHAN_AGE_SECS;

    let mut stmt = conn.prepare(
        "SELECT id, uuid FROM attachment
         WHERE entity_type = 'asset'
           AND entity_id IS NULL
           AND created_at < ?1
           AND deleted_at IS NULL
           AND uuid NOT IN (
               SELECT hero_attachment_uuid FROM asset
               WHERE hero_attachment_uuid IS NOT NULL
           )",
    )?;

    let rows: Vec<(i64, String)> = stmt
        .query_map(params![cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(Result::ok)
        .collect();

    let mut swept = 0usize;
    for (id, uuid) in rows {
        let file = attachments_dir.join(&uuid);
        let _ = std::fs::remove_file(&file);
        conn.execute("DELETE FROM attachment WHERE id = ?1", params![id])?;
        swept += 1;
    }

    Ok(swept)
}
