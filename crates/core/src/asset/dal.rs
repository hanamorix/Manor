//! Asset DAL — CRUD with soft-delete + filter.

use super::{Asset, AssetCategory, AssetDraft};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

#[derive(Debug, Clone, Default)]
pub struct AssetListFilter {
    pub search: Option<String>,
    pub category: Option<AssetCategory>,
    pub include_trashed: bool,
}

pub fn insert_asset(conn: &Connection, draft: &AssetDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    conn.execute(
        "INSERT INTO asset (id, name, category, make, model, serial_number, purchase_date, notes,
                             hero_attachment_uuid, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            id, draft.name, draft.category.as_str(),
            draft.make, draft.model, draft.serial_number, draft.purchase_date,
            draft.notes, draft.hero_attachment_uuid, now,
        ],
    )?;
    Ok(id)
}

pub fn get_asset(conn: &Connection, id: &str) -> Result<Option<Asset>> {
    select_one(conn, id, /*include_trashed=*/ false)
}

pub fn get_asset_including_trashed(conn: &Connection, id: &str) -> Result<Option<Asset>> {
    select_one(conn, id, /*include_trashed=*/ true)
}

fn select_one(conn: &Connection, id: &str, include_trashed: bool) -> Result<Option<Asset>> {
    let mut sql = String::from(
        "SELECT id, name, category, make, model, serial_number, purchase_date, notes,
                hero_attachment_uuid, created_at, updated_at, deleted_at
         FROM asset WHERE id = ?1",
    );
    if !include_trashed {
        sql.push_str(" AND deleted_at IS NULL");
    }
    let mut stmt = conn.prepare(&sql)?;
    let row = stmt.query_row(params![id], row_to_asset).optional()?;
    Ok(row)
}

pub fn list_assets(conn: &Connection, filter: &AssetListFilter) -> Result<Vec<Asset>> {
    let mut sql = String::from(
        "SELECT id, name, category, make, model, serial_number, purchase_date, notes,
                hero_attachment_uuid, created_at, updated_at, deleted_at
         FROM asset WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if !filter.include_trashed {
        sql.push_str(" AND deleted_at IS NULL");
    }
    if let Some(q) = filter.search.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND name LIKE ?");
        params.push(Box::new(format!("%{}%", q)));
    }
    if let Some(c) = filter.category {
        sql.push_str(" AND category = ?");
        params.push(Box::new(c.as_str().to_string()));
    }
    sql.push_str(" ORDER BY name COLLATE NOCASE ASC");

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_asset)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn update_asset(conn: &Connection, id: &str, draft: &AssetDraft) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE asset SET name = ?1, category = ?2, make = ?3, model = ?4, serial_number = ?5,
                          purchase_date = ?6, notes = ?7, hero_attachment_uuid = ?8, updated_at = ?9
         WHERE id = ?10",
        params![
            draft.name, draft.category.as_str(),
            draft.make, draft.model, draft.serial_number, draft.purchase_date,
            draft.notes, draft.hero_attachment_uuid, now, id,
        ],
    )?;
    Ok(())
}

pub fn soft_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE asset SET deleted_at = ?1 WHERE id = ?2", params![now_secs(), id])?;
    Ok(())
}

pub fn restore_asset(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE asset SET deleted_at = NULL WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_hero_attachment(conn: &Connection, id: &str, uuid: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE asset SET hero_attachment_uuid = ?1, updated_at = ?2 WHERE id = ?3",
        params![uuid, now_secs(), id],
    )?;
    Ok(())
}

/// Hard-delete a trashed asset. Also soft-deletes its document + hero attachments
/// so the attachment sweeper cleans up disk files. Only affects rows that are
/// already soft-deleted.
pub fn permanent_delete_asset(conn: &Connection, id: &str) -> Result<()> {
    // Soft-delete linked attachments (the attachment permanent-delete sweep
    // removes files later).
    let now = now_secs();
    conn.execute(
        "UPDATE attachment SET deleted_at = ?1 WHERE entity_type = 'asset' AND entity_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    // Hard-delete the asset (only if trashed).
    conn.execute(
        "DELETE FROM asset WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}

fn row_to_asset(row: &rusqlite::Row) -> rusqlite::Result<Asset> {
    let category: String = row.get(2)?;
    Ok(Asset {
        id: row.get(0)?,
        name: row.get(1)?,
        category: AssetCategory::from_db(&category),
        make: row.get(3)?,
        model: row.get(4)?,
        serial_number: row.get(5)?,
        purchase_date: row.get(6)?,
        notes: row.get(7)?,
        hero_attachment_uuid: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        deleted_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn draft(name: &str, cat: AssetCategory) -> AssetDraft {
        AssetDraft {
            name: name.into(),
            category: cat,
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        }
    }

    #[test]
    fn insert_and_get_roundtrips_with_all_optional_fields_null() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
        let got = get_asset(&conn, &id).unwrap().unwrap();
        assert_eq!(got.name, "Boiler");
        assert_eq!(got.category, AssetCategory::Appliance);
        assert!(got.make.is_none());
        assert!(got.model.is_none());
        assert!(got.hero_attachment_uuid.is_none());
    }

    #[test]
    fn update_replaces_fields_cleanly_including_clearing_to_none() {
        let (_d, conn) = fresh();
        let mut d = draft("Boiler", AssetCategory::Appliance);
        d.make = Some("Worcester".into());
        d.serial_number = Some("123".into());
        let id = insert_asset(&conn, &d).unwrap();

        let mut d2 = draft("Boiler", AssetCategory::Appliance);
        d2.make = None;
        d2.serial_number = None;
        update_asset(&conn, &id, &d2).unwrap();

        let got = get_asset(&conn, &id).unwrap().unwrap();
        assert!(got.make.is_none());
        assert!(got.serial_number.is_none());
    }

    #[test]
    fn get_asset_hides_trashed_get_including_surfaces_them() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("Gone", AssetCategory::Other)).unwrap();
        soft_delete_asset(&conn, &id).unwrap();
        assert!(get_asset(&conn, &id).unwrap().is_none());
        let ghost = get_asset_including_trashed(&conn, &id).unwrap().unwrap();
        assert!(ghost.deleted_at.is_some());
    }

    #[test]
    fn list_filters_by_search_and_category() {
        let (_d, conn) = fresh();
        insert_asset(&conn, &draft("Boiler", AssetCategory::Appliance)).unwrap();
        insert_asset(&conn, &draft("Honda Civic", AssetCategory::Vehicle)).unwrap();
        insert_asset(&conn, &draft("Dishwasher", AssetCategory::Appliance)).unwrap();

        let all = list_assets(&conn, &AssetListFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        let appliances = list_assets(&conn, &AssetListFilter {
            category: Some(AssetCategory::Appliance),
            ..Default::default()
        }).unwrap();
        assert_eq!(appliances.len(), 2);

        let search = list_assets(&conn, &AssetListFilter {
            search: Some("boil".into()),
            ..Default::default()
        }).unwrap();
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].name, "Boiler");
    }

    #[test]
    fn list_orders_alphabetical_case_insensitive() {
        let (_d, conn) = fresh();
        insert_asset(&conn, &draft("zebra", AssetCategory::Other)).unwrap();
        insert_asset(&conn, &draft("Apple", AssetCategory::Other)).unwrap();
        insert_asset(&conn, &draft("banana", AssetCategory::Other)).unwrap();
        let list = list_assets(&conn, &AssetListFilter::default()).unwrap();
        let names: Vec<_> = list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Apple", "banana", "zebra"]);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("X", AssetCategory::Other)).unwrap();
        soft_delete_asset(&conn, &id).unwrap();
        restore_asset(&conn, &id).unwrap();
        assert!(get_asset(&conn, &id).unwrap().is_some());
    }

    #[test]
    fn set_hero_attachment_updates_field() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("X", AssetCategory::Other)).unwrap();
        set_hero_attachment(&conn, &id, Some("uuid-123")).unwrap();
        assert_eq!(get_asset(&conn, &id).unwrap().unwrap().hero_attachment_uuid.as_deref(), Some("uuid-123"));
        set_hero_attachment(&conn, &id, None).unwrap();
        assert!(get_asset(&conn, &id).unwrap().unwrap().hero_attachment_uuid.is_none());
    }

    #[test]
    fn permanent_delete_removes_trashed_asset_and_soft_deletes_attachments() {
        let (_d, conn) = fresh();
        let id = insert_asset(&conn, &draft("Gone", AssetCategory::Other)).unwrap();

        // Pre-seed a linked attachment row directly (skipping the file-copy path).
        conn.execute(
            "INSERT INTO attachment (uuid, original_name, mime_type, size_bytes, sha256,
                                      entity_type, entity_id)
             VALUES ('att-uuid', 'manual.pdf', 'application/pdf', 100, 'abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123abc1', 'asset', ?1)",
            rusqlite::params![id],
        ).unwrap();

        // Active (not trashed) assets are NOT affected by permanent_delete.
        permanent_delete_asset(&conn, &id).unwrap();
        assert!(get_asset(&conn, &id).unwrap().is_some());

        // Trash first, then permanent-delete.
        soft_delete_asset(&conn, &id).unwrap();
        permanent_delete_asset(&conn, &id).unwrap();
        assert!(get_asset_including_trashed(&conn, &id).unwrap().is_none());

        // Attachment row should be soft-deleted (deleted_at IS NOT NULL).
        let att_trashed: Option<i64> = conn.query_row(
            "SELECT deleted_at FROM attachment WHERE entity_type = 'asset' AND entity_id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        ).optional().unwrap().flatten();
        assert!(att_trashed.is_some(), "attachment should be soft-deleted post-purge");
    }
}
