//! Startup sweep: remove crash-orphan staged recipe attachments.
//!
//! When `recipe_import_commit` stages a hero image but dies before linking it,
//! a row with `entity_type='recipe'` and `entity_id IS NULL` is left behind.
//! This sweep runs at app start and hard-deletes orphans older than 24 h,
//! along with their files on disk.

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

const ORPHAN_AGE_SECS: i64 = 24 * 60 * 60;

/// Remove staged recipe attachments (entity_type='recipe', entity_id IS NULL)
/// older than 24 h that are NOT referenced by any recipe's hero_attachment_uuid.
///
/// The exclusion subquery is the canonical "linked" signal: after a successful
/// import, `recipe.hero_attachment_uuid` points at the attachment's uuid, and
/// `entity_id` stays NULL in the attachment table (avoiding the INTEGER/TEXT
/// mismatch). Returns the number of orphans swept.
pub fn run_on_startup(conn: &Connection, attachments_dir: &Path) -> Result<usize> {
    let cutoff = chrono::Utc::now().timestamp() - ORPHAN_AGE_SECS;

    let mut stmt = conn.prepare(
        "SELECT id, uuid FROM attachment
         WHERE entity_type = 'recipe'
           AND entity_id IS NULL
           AND created_at < ?1
           AND deleted_at IS NULL
           AND uuid NOT IN (
               SELECT hero_attachment_uuid FROM recipe
               WHERE hero_attachment_uuid IS NOT NULL
           )",
    )?;

    let rows: Vec<(i64, String)> = stmt
        .query_map(params![cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(Result::ok)
        .collect();

    let mut swept = 0usize;
    for (id, uuid) in rows {
        // Best-effort file removal — use the same uuid-based path as attachment::file_path.
        let file = attachments_dir.join(&uuid);
        let _ = std::fs::remove_file(&file);
        // Hard-delete the DB row (orphans were never linked; skip trash treatment).
        conn.execute("DELETE FROM attachment WHERE id = ?1", params![id])?;
        swept += 1;
    }

    Ok(swept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_db(dir: &TempDir) -> Connection {
        manor_core::assistant::db::init(&dir.path().join("t.db")).unwrap()
    }

    #[test]
    fn sweeps_orphans_older_than_24h() {
        let dir = TempDir::new().unwrap();
        let conn = fresh_db(&dir);

        // Insert a staged recipe attachment with created_at > 48 h ago.
        let old_ts = chrono::Utc::now().timestamp() - 48 * 60 * 60;
        let uuid = "test-uuid-old";
        conn.execute(
            "INSERT INTO attachment
             (uuid, original_name, mime_type, size_bytes, sha256,
              entity_type, entity_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            params![
                uuid,
                "hero.jpg",
                "image/jpeg",
                42i64,
                "deadbeef",
                "recipe",
                old_ts,
            ],
        )
        .unwrap();

        // Put a fake file on disk so we can assert it's removed.
        let att_dir = dir.path().join("attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        std::fs::write(att_dir.join(uuid), b"fake image bytes").unwrap();

        let swept = run_on_startup(&conn, &att_dir).unwrap();
        assert_eq!(swept, 1);
        assert!(!att_dir.join(uuid).exists(), "file should be removed");

        // Row gone from DB.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM attachment", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn does_not_sweep_recent_orphans() {
        let dir = TempDir::new().unwrap();
        let conn = fresh_db(&dir);

        // created_at = now (recent)
        let now_ts = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO attachment
             (uuid, original_name, mime_type, size_bytes, sha256,
              entity_type, entity_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            params!["recent-uuid", "hero.png", "image/png", 10i64, "abc123", "recipe", now_ts],
        )
        .unwrap();

        let att_dir = dir.path().join("attachments");
        let swept = run_on_startup(&conn, &att_dir).unwrap();
        assert_eq!(swept, 0);
    }

    #[test]
    fn does_not_sweep_linked_attachments() {
        let dir = TempDir::new().unwrap();
        let conn = fresh_db(&dir);

        // Attachment staged (entity_id IS NULL) but referenced by a recipe via
        // hero_attachment_uuid — should NOT be swept even if old.
        let old_ts = chrono::Utc::now().timestamp() - 48 * 60 * 60;
        let att_uuid = "linked-hero-uuid";
        conn.execute(
            "INSERT INTO attachment
             (uuid, original_name, mime_type, size_bytes, sha256,
              entity_type, entity_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            params![
                att_uuid,
                "hero.jpg",
                "image/jpeg",
                99i64,
                "cafebabe",
                "recipe",
                old_ts,
            ],
        )
        .unwrap();

        // Insert a recipe that references this attachment's uuid.
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO recipe
             (id, title, instructions, import_method, created_at, updated_at, hero_attachment_uuid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            params![
                "some-recipe-uuid",
                "Test recipe",
                "Cook it.",
                "manual",
                now,
                att_uuid,
            ],
        )
        .unwrap();

        let att_dir = dir.path().join("attachments");
        let swept = run_on_startup(&conn, &att_dir).unwrap();
        assert_eq!(swept, 0);
    }
}
