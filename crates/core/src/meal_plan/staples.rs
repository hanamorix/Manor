//! Staple CRUD. Aliases stored as JSON array in the TEXT `aliases` column.

use super::{StapleDraft, StapleItem};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn list_staples(conn: &Connection) -> Result<Vec<StapleItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, aliases, created_at, updated_at, deleted_at
         FROM staple_item WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        let aliases_json: Option<String> = r.get(2)?;
        let aliases: Vec<String> = aliases_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Ok(StapleItem {
            id: r.get(0)?,
            name: r.get(1)?,
            aliases,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
            deleted_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_staple(conn: &Connection, id: &str) -> Result<Option<StapleItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, aliases, created_at, updated_at, deleted_at
         FROM staple_item WHERE id = ?1",
    )?;
    stmt.query_row(params![id], |r| {
        let aliases_json: Option<String> = r.get(2)?;
        let aliases: Vec<String> = aliases_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Ok(StapleItem {
            id: r.get(0)?,
            name: r.get(1)?,
            aliases,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
            deleted_at: r.get(5)?,
        })
    })
    .optional()
    .map_err(Into::into)
}

pub fn insert_staple(conn: &Connection, draft: &StapleDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let aliases_json = if draft.aliases.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&draft.aliases)?)
    };
    conn.execute(
        "INSERT INTO staple_item (id, name, aliases, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, draft.name, aliases_json, now],
    )?;
    Ok(id)
}

pub fn update_staple(conn: &Connection, id: &str, draft: &StapleDraft) -> Result<()> {
    let now = now_secs();
    let aliases_json = if draft.aliases.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&draft.aliases)?)
    };
    conn.execute(
        "UPDATE staple_item SET name = ?1, aliases = ?2, updated_at = ?3 WHERE id = ?4",
        params![draft.name, aliases_json, now, id],
    )?;
    Ok(())
}

pub fn soft_delete_staple(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE staple_item SET deleted_at = ?1 WHERE id = ?2",
        params![now_secs(), id],
    )?;
    Ok(())
}

pub fn restore_staple(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE staple_item SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Hard-delete a staple. Intended for trash-permanent-delete.
/// Only removes rows that are already soft-deleted.
pub fn permanent_delete_staple(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM staple_item WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
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

    #[test]
    fn crud_with_aliases_roundtrip() {
        let (_d, conn) = fresh();
        let id = insert_staple(
            &conn,
            &StapleDraft {
                name: "Olive oil".into(),
                aliases: vec!["EVOO".into(), "extra virgin olive oil".into()],
            },
        )
        .unwrap();
        let got = get_staple(&conn, &id).unwrap().unwrap();
        assert_eq!(got.name, "Olive oil");
        assert_eq!(got.aliases, vec!["EVOO", "extra virgin olive oil"]);
    }

    #[test]
    fn list_excludes_trashed_and_sorts_by_name_ci() {
        let (_d, conn) = fresh();
        insert_staple(
            &conn,
            &StapleDraft {
                name: "salt".into(),
                aliases: vec![],
            },
        )
        .unwrap();
        let id = insert_staple(
            &conn,
            &StapleDraft {
                name: "Olive oil".into(),
                aliases: vec![],
            },
        )
        .unwrap();
        insert_staple(
            &conn,
            &StapleDraft {
                name: "Garlic".into(),
                aliases: vec![],
            },
        )
        .unwrap();
        soft_delete_staple(&conn, &id).unwrap();
        let list = list_staples(&conn).unwrap();
        let names: Vec<_> = list.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Garlic", "salt"]);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn) = fresh();
        let id = insert_staple(
            &conn,
            &StapleDraft {
                name: "salt".into(),
                aliases: vec![],
            },
        )
        .unwrap();
        soft_delete_staple(&conn, &id).unwrap();
        assert!(list_staples(&conn).unwrap().is_empty());
        restore_staple(&conn, &id).unwrap();
        assert_eq!(list_staples(&conn).unwrap().len(), 1);
    }

    #[test]
    fn permanent_delete_only_removes_trashed() {
        let (_d, conn) = fresh();
        let id = insert_staple(&conn, &StapleDraft { name: "salt".into(), aliases: vec![] }).unwrap();
        // active rows are not removable via permanent_delete
        permanent_delete_staple(&conn, &id).unwrap();
        assert!(get_staple(&conn, &id).unwrap().is_some());
        // trashed rows get hard-deleted
        soft_delete_staple(&conn, &id).unwrap();
        permanent_delete_staple(&conn, &id).unwrap();
        assert!(get_staple(&conn, &id).unwrap().is_none());
    }

    #[test]
    fn update_replaces_aliases() {
        let (_d, conn) = fresh();
        let id = insert_staple(
            &conn,
            &StapleDraft {
                name: "Olive oil".into(),
                aliases: vec!["EVOO".into()],
            },
        )
        .unwrap();
        update_staple(
            &conn,
            &id,
            &StapleDraft {
                name: "Olive oil".into(),
                aliases: vec![],
            },
        )
        .unwrap();
        let got = get_staple(&conn, &id).unwrap().unwrap();
        assert!(got.aliases.is_empty());
    }
}
