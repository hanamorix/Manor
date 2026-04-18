//! Attachment DAL — files stored at <root>/<uuid>, metadata in SQLite.
//! Dedup by sha256: duplicate bytes reuse the same file on disk.

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub id: i64,
    pub uuid: String,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub created_at: i64,
}

impl Attachment {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            uuid: row.get("uuid")?,
            original_name: row.get("original_name")?,
            mime_type: row.get("mime_type")?,
            size_bytes: row.get("size_bytes")?,
            sha256: row.get("sha256")?,
            entity_type: row.get("entity_type")?,
            entity_id: row.get("entity_id")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Where attachment bytes live, relative to the app data dir.
/// The concrete path is provided by the caller (app layer knows `app_data_dir()`).
pub fn file_path(root: &Path, uuid: &str) -> PathBuf {
    root.join(uuid)
}

fn compute_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Store bytes on disk + insert a row. If sha256 already exists on a non-deleted
/// row, reuse its `uuid` (no new file written, metadata-only row added).
pub fn store(
    conn: &Connection,
    root: &Path,
    bytes: &[u8],
    original_name: &str,
    mime_type: &str,
    entity_type: Option<&str>,
    entity_id: Option<i64>,
) -> Result<Attachment> {
    std::fs::create_dir_all(root)?;
    let sha = compute_sha256(bytes);

    // Dedup: reuse an existing non-deleted row's uuid if sha matches.
    let existing_uuid: Option<String> = conn
        .query_row(
            "SELECT uuid FROM attachment WHERE sha256 = ?1 AND deleted_at IS NULL LIMIT 1",
            [&sha],
            |r| r.get(0),
        )
        .ok();

    let uuid = match existing_uuid {
        Some(u) => u,
        None => {
            let new_uuid = Uuid::new_v4().to_string();
            let path = file_path(root, &new_uuid);
            std::fs::write(&path, bytes)?;
            new_uuid
        }
    };

    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO attachment
         (uuid, original_name, mime_type, size_bytes, sha256, entity_type, entity_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            uuid,
            original_name,
            mime_type,
            bytes.len() as i64,
            sha,
            entity_type,
            entity_id,
            now
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Attachment> {
    let mut stmt = conn.prepare(
        "SELECT id, uuid, original_name, mime_type, size_bytes, sha256,
                entity_type, entity_id, created_at
         FROM attachment WHERE id = ?1 AND deleted_at IS NULL",
    )?;
    Ok(stmt.query_row([id], Attachment::from_row)?)
}

pub fn get_bytes(conn: &Connection, root: &Path, id: i64) -> Result<Vec<u8>> {
    let row = get(conn, id)?;
    let p = file_path(root, &row.uuid);
    Ok(std::fs::read(&p)?)
}

pub fn list_for(conn: &Connection, entity_type: &str, entity_id: i64) -> Result<Vec<Attachment>> {
    let mut stmt = conn.prepare(
        "SELECT id, uuid, original_name, mime_type, size_bytes, sha256,
                entity_type, entity_id, created_at
         FROM attachment
         WHERE entity_type = ?1 AND entity_id = ?2 AND deleted_at IS NULL
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![entity_type, entity_id], Attachment::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Soft-delete. File is cleaned up by the permanent-delete sweep (Phase B).
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE attachment SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Hard-delete + file cleanup. Only removes the file if no other non-deleted row
/// shares the uuid (dedup protection).
pub fn permanent_delete(conn: &Connection, root: &Path, id: i64) -> Result<()> {
    let uuid: String = conn.query_row("SELECT uuid FROM attachment WHERE id = ?1", [id], |r| {
        r.get(0)
    })?;
    conn.execute("DELETE FROM attachment WHERE id = ?1", [id])?;
    let still_referenced: i64 = conn.query_row(
        "SELECT COUNT(*) FROM attachment WHERE uuid = ?1 AND deleted_at IS NULL",
        [&uuid],
        |r| r.get(0),
    )?;
    if still_referenced == 0 {
        let p = file_path(root, &uuid);
        if p.exists() {
            std::fs::remove_file(&p)
                .map_err(|e| anyhow!("failed to remove file {}: {e}", p.display()))?;
        }
    }
    Ok(())
}

pub fn restore(conn: &Connection, id: i64) -> Result<Attachment> {
    conn.execute(
        "UPDATE attachment SET deleted_at = NULL WHERE id = ?1",
        [id],
    )?;
    get(conn, id)
}

/// Link a staged attachment (entity_id IS NULL) to its owning entity.
///
/// `entity_id_str` is stored in the `entity_id` column via SQLite's dynamic
/// typing; recipes use TEXT UUIDs while most entities use integers.
/// The sweep queries `entity_id IS NULL`, so this is only needed post-commit.
pub fn link_to_entity(
    conn: &Connection,
    attachment_id: i64,
    entity_type: &str,
    entity_id_str: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE attachment SET entity_type = ?1, entity_id = ?2 WHERE id = ?3",
        params![entity_type, entity_id_str, attachment_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_env() -> (tempfile::TempDir, Connection, PathBuf) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let root = dir.path().join("attachments");
        (dir, conn, root)
    }

    #[test]
    fn store_writes_file_and_row() {
        let (_d, conn, root) = fresh_env();
        let a = store(&conn, &root, b"hello", "hi.txt", "text/plain", None, None).unwrap();
        assert_eq!(a.original_name, "hi.txt");
        assert_eq!(a.size_bytes, 5);
        assert_eq!(a.sha256.len(), 64);
        let disk = std::fs::read(file_path(&root, &a.uuid)).unwrap();
        assert_eq!(disk, b"hello");
    }

    #[test]
    fn store_dedups_by_sha() {
        let (_d, conn, root) = fresh_env();
        let a = store(&conn, &root, b"same", "a.txt", "text/plain", None, None).unwrap();
        let b = store(&conn, &root, b"same", "b.txt", "text/plain", None, None).unwrap();
        assert_eq!(a.uuid, b.uuid);
        assert_ne!(a.id, b.id);
        // Only one file on disk.
        let entries: Vec<_> = std::fs::read_dir(&root).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn get_bytes_reads_the_file() {
        let (_d, conn, root) = fresh_env();
        let a = store(
            &conn,
            &root,
            b"xyz",
            "x.bin",
            "application/octet-stream",
            None,
            None,
        )
        .unwrap();
        let bytes = get_bytes(&conn, &root, a.id).unwrap();
        assert_eq!(bytes, b"xyz");
    }

    #[test]
    fn list_for_filters_by_entity() {
        let (_d, conn, root) = fresh_env();
        store(
            &conn,
            &root,
            b"a",
            "a.txt",
            "text/plain",
            Some("task"),
            Some(1),
        )
        .unwrap();
        store(
            &conn,
            &root,
            b"b",
            "b.txt",
            "text/plain",
            Some("task"),
            Some(1),
        )
        .unwrap();
        store(
            &conn,
            &root,
            b"c",
            "c.txt",
            "text/plain",
            Some("task"),
            Some(2),
        )
        .unwrap();
        assert_eq!(list_for(&conn, "task", 1).unwrap().len(), 2);
    }

    #[test]
    fn soft_delete_hides_row_but_keeps_file() {
        let (_d, conn, root) = fresh_env();
        let a = store(&conn, &root, b"keep", "k.txt", "text/plain", None, None).unwrap();
        delete(&conn, a.id).unwrap();
        assert!(get(&conn, a.id).is_err());
        assert!(file_path(&root, &a.uuid).exists());
    }

    #[test]
    fn permanent_delete_removes_file_when_no_refs() {
        let (_d, conn, root) = fresh_env();
        let a = store(&conn, &root, b"gone", "g.txt", "text/plain", None, None).unwrap();
        permanent_delete(&conn, &root, a.id).unwrap();
        assert!(!file_path(&root, &a.uuid).exists());
    }

    #[test]
    fn permanent_delete_keeps_file_when_other_rows_reference_uuid() {
        let (_d, conn, root) = fresh_env();
        let a = store(&conn, &root, b"shared", "a.txt", "text/plain", None, None).unwrap();
        let b = store(&conn, &root, b"shared", "b.txt", "text/plain", None, None).unwrap();
        assert_eq!(a.uuid, b.uuid);
        permanent_delete(&conn, &root, a.id).unwrap();
        assert!(
            file_path(&root, &a.uuid).exists(),
            "file must remain — b still references it"
        );
        // Now remove the other reference; file goes.
        permanent_delete(&conn, &root, b.id).unwrap();
        assert!(!file_path(&root, &a.uuid).exists());
    }

    #[test]
    fn restore_brings_back_row() {
        let (_d, conn, root) = fresh_env();
        let a = store(&conn, &root, b"x", "x.txt", "text/plain", None, None).unwrap();
        delete(&conn, a.id).unwrap();
        restore(&conn, a.id).unwrap();
        assert!(get(&conn, a.id).is_ok());
    }
}
