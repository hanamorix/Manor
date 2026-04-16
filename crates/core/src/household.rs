//! Household singleton DAL. Exactly one row, id=1, seeded by V8 migration.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type WorkingHours = HashMap<String, Vec<i64>>; // day -> [start_hour, end_hour] (empty = rest day)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DndWindow {
    pub day: String,
    pub start_hour: i64,
    pub end_hour: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Household {
    pub owner_person_id: Option<i64>,
    pub working_hours: WorkingHours,
    pub dnd_windows: Vec<DndWindow>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn get(conn: &Connection) -> Result<Household> {
    let (owner_id, wh_json, dnd_json, created_at, updated_at): (Option<i64>, String, String, i64, i64) =
        conn.query_row(
            "SELECT owner_person_id, working_hours_json, dnd_windows_json, created_at, updated_at
             FROM household WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    Ok(Household {
        owner_person_id: owner_id,
        working_hours: serde_json::from_str(&wh_json)?,
        dnd_windows: serde_json::from_str(&dnd_json)?,
        created_at,
        updated_at,
    })
}

pub fn set_owner(conn: &Connection, owner_person_id: Option<i64>) -> Result<Household> {
    conn.execute(
        "UPDATE household SET owner_person_id = ?1, updated_at = unixepoch() WHERE id = 1",
        params![owner_person_id],
    )?;
    get(conn)
}

pub fn set_working_hours(conn: &Connection, hours: &WorkingHours) -> Result<Household> {
    let j = serde_json::to_string(hours)?;
    conn.execute(
        "UPDATE household SET working_hours_json = ?1, updated_at = unixepoch() WHERE id = 1",
        params![j],
    )?;
    get(conn)
}

pub fn set_dnd_windows(conn: &Connection, windows: &[DndWindow]) -> Result<Household> {
    let j = serde_json::to_string(windows)?;
    conn.execute(
        "UPDATE household SET dnd_windows_json = ?1, updated_at = unixepoch() WHERE id = 1",
        params![j],
    )?;
    get(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn get_returns_seeded_singleton() {
        let (_d, conn) = fresh_conn();
        let h = get(&conn).unwrap();
        assert!(h.owner_person_id.is_none());
        assert_eq!(h.working_hours.get("mon"), Some(&vec![9, 17]));
        assert_eq!(h.working_hours.get("sat"), Some(&vec![]));
        assert!(h.dnd_windows.is_empty());
    }

    #[test]
    fn set_owner_roundtrips() {
        let (_d, conn) = fresh_conn();
        let person_id = crate::person::insert(&conn, "Hana", "owner", None, None, None).unwrap().id;
        let h = set_owner(&conn, Some(person_id)).unwrap();
        assert_eq!(h.owner_person_id, Some(person_id));
    }

    #[test]
    fn set_working_hours_persists() {
        let (_d, conn) = fresh_conn();
        let mut hrs: WorkingHours = HashMap::new();
        hrs.insert("mon".into(), vec![8, 16]);
        hrs.insert("tue".into(), vec![]);
        set_working_hours(&conn, &hrs).unwrap();
        let h = get(&conn).unwrap();
        assert_eq!(h.working_hours.get("mon"), Some(&vec![8, 16]));
        assert_eq!(h.working_hours.get("tue"), Some(&vec![]));
    }

    #[test]
    fn set_dnd_windows_persists() {
        let (_d, conn) = fresh_conn();
        let w = vec![DndWindow { day: "fri".into(), start_hour: 18, end_hour: 23 }];
        set_dnd_windows(&conn, &w).unwrap();
        let h = get(&conn).unwrap();
        assert_eq!(h.dnd_windows.len(), 1);
        assert_eq!(h.dnd_windows[0].day, "fri");
    }
}
