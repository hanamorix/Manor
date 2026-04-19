//! Maintenance schedule DAL: CRUD, mark_done, and band-query helpers.

use super::{due, MaintenanceSchedule, MaintenanceScheduleDraft};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn today_local() -> String {
    chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()
}

pub fn insert_schedule(conn: &Connection, draft: &MaintenanceScheduleDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let today = today_local();
    let next_due = due::compute_next_due(draft.last_done_date.as_deref(), draft.interval_months, &today)?;
    conn.execute(
        "INSERT INTO maintenance_schedule
           (id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id, draft.asset_id, draft.task, draft.interval_months,
            draft.last_done_date, next_due, draft.notes, now,
        ],
    )?;
    Ok(id)
}

pub fn get_schedule(conn: &Connection, id: &str) -> Result<Option<MaintenanceSchedule>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
                created_at, updated_at, deleted_at
         FROM maintenance_schedule WHERE id = ?1 AND deleted_at IS NULL",
    )?;
    stmt.query_row(params![id], row_to_schedule).optional().map_err(Into::into)
}

pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<MaintenanceSchedule>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
                created_at, updated_at, deleted_at
         FROM maintenance_schedule
         WHERE asset_id = ?1 AND deleted_at IS NULL
         ORDER BY next_due_date ASC",
    )?;
    let rows = stmt.query_map(params![asset_id], row_to_schedule)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn list_due_before(conn: &Connection, cutoff_date: &str) -> Result<Vec<MaintenanceSchedule>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
                created_at, updated_at, deleted_at
         FROM maintenance_schedule
         WHERE next_due_date <= ?1 AND deleted_at IS NULL
         ORDER BY next_due_date ASC",
    )?;
    let rows = stmt.query_map(params![cutoff_date], row_to_schedule)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn list_due_today_and_overdue(conn: &Connection, today: &str) -> Result<Vec<MaintenanceSchedule>> {
    // "Due today" and "overdue" both satisfy next_due_date <= today.
    list_due_before(conn, today)
}

pub fn overdue_count(conn: &Connection, today: &str) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM maintenance_schedule
         WHERE next_due_date <= ?1 AND deleted_at IS NULL",
        params![today],
        |r| r.get(0),
    )?;
    Ok(count)
}

pub fn update_schedule(conn: &Connection, id: &str, draft: &MaintenanceScheduleDraft) -> Result<()> {
    let now = now_secs();
    let today = today_local();
    let next_due = due::compute_next_due(draft.last_done_date.as_deref(), draft.interval_months, &today)?;
    conn.execute(
        "UPDATE maintenance_schedule
         SET asset_id = ?1, task = ?2, interval_months = ?3,
             last_done_date = ?4, next_due_date = ?5, notes = ?6, updated_at = ?7
         WHERE id = ?8",
        params![
            draft.asset_id, draft.task, draft.interval_months,
            draft.last_done_date, next_due, draft.notes, now, id,
        ],
    )?;
    Ok(())
}

pub fn mark_done(conn: &Connection, id: &str, today: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT interval_months FROM maintenance_schedule WHERE id = ?1 AND deleted_at IS NULL",
    )?;
    let interval: i32 = stmt.query_row(params![id], |r| r.get(0))?;
    let next_due = due::compute_next_due(Some(today), interval, today)?;
    let now = now_secs();
    conn.execute(
        "UPDATE maintenance_schedule
         SET last_done_date = ?1, next_due_date = ?2, updated_at = ?3
         WHERE id = ?4",
        params![today, next_due, now, id],
    )?;
    Ok(())
}

pub fn soft_delete_schedule(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = ?1 WHERE id = ?2",
        params![now_secs(), id],
    )?;
    Ok(())
}

pub fn restore_schedule(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE maintenance_schedule SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn permanent_delete_schedule(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM maintenance_schedule WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    Ok(())
}

fn row_to_schedule(row: &rusqlite::Row) -> rusqlite::Result<MaintenanceSchedule> {
    Ok(MaintenanceSchedule {
        id: row.get(0)?,
        asset_id: row.get(1)?,
        task: row.get(2)?,
        interval_months: row.get(3)?,
        last_done_date: row.get(4)?,
        next_due_date: row.get(5)?,
        notes: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deleted_at: row.get(9)?,
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
            make: None, model: None, serial_number: None, purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_id = asset_dal::insert_asset(&conn, &asset).unwrap();
        (dir, conn, asset_id)
    }

    fn simple_draft(asset_id: &str) -> MaintenanceScheduleDraft {
        MaintenanceScheduleDraft {
            asset_id: asset_id.into(),
            task: "Annual service".into(),
            interval_months: 12,
            last_done_date: None,
            notes: String::new(),
        }
    }

    #[test]
    fn insert_and_get_populates_next_due_from_fallback() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.task, "Annual service");
        assert!(s.last_done_date.is_none());
        // next_due = today + 12 months; don't hardcode — just verify non-empty + YYYY-MM-DD format.
        assert_eq!(s.next_due_date.len(), 10);
        assert!(s.next_due_date.chars().nth(4) == Some('-'));
    }

    #[test]
    fn insert_with_last_done_uses_that_as_anchor() {
        let (_d, conn, asset_id) = fresh();
        let mut draft = simple_draft(&asset_id);
        draft.last_done_date = Some("2024-08-15".into());
        let id = insert_schedule(&conn, &draft).unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.next_due_date, "2025-08-15");
    }

    #[test]
    fn update_recomputes_next_due_when_interval_changes() {
        let (_d, conn, asset_id) = fresh();
        let mut draft = simple_draft(&asset_id);
        draft.last_done_date = Some("2024-08-15".into());
        let id = insert_schedule(&conn, &draft).unwrap();

        draft.interval_months = 24;
        update_schedule(&conn, &id, &draft).unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.next_due_date, "2026-08-15");
    }

    #[test]
    fn mark_done_bumps_both_dates() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        mark_done(&conn, &id, "2025-06-15").unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.last_done_date.as_deref(), Some("2025-06-15"));
        assert_eq!(s.next_due_date, "2026-06-15");
    }

    #[test]
    fn list_for_asset_excludes_trashed_and_orders_by_due() {
        let (_d, conn, asset_id) = fresh();
        let a = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task A".into();
            d.last_done_date = Some("2024-06-15".into());  // next_due 2025-06-15
            d
        }).unwrap();
        let b = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task B".into();
            d.last_done_date = Some("2024-01-15".into());  // next_due 2025-01-15
            d
        }).unwrap();
        let c = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task C".into();
            d.last_done_date = Some("2024-03-15".into());  // next_due 2025-03-15
            d
        }).unwrap();
        soft_delete_schedule(&conn, &c).unwrap();

        let list = list_for_asset(&conn, &asset_id).unwrap();
        let tasks: Vec<_> = list.iter().map(|s| s.task.as_str()).collect();
        assert_eq!(tasks, vec!["Task B", "Task A"]);
        let _ = (a, b);
    }

    #[test]
    fn list_due_before_includes_overdue_and_cutoff_day() {
        let (_d, conn, asset_id) = fresh();
        insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Past".into();
            d.last_done_date = Some("2023-06-15".into());  // next_due 2024-06-15
            d
        }).unwrap();
        insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Future".into();
            d.last_done_date = Some("2026-01-15".into());  // next_due 2027-01-15
            d
        }).unwrap();

        let list = list_due_before(&conn, "2025-01-01").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].task, "Past");
    }

    #[test]
    fn overdue_count_respects_trashed() {
        let (_d, conn, asset_id) = fresh();
        insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.last_done_date = Some("2023-06-15".into());
            d
        }).unwrap();
        let id2 = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.last_done_date = Some("2023-07-15".into());
            d
        }).unwrap();
        assert_eq!(overdue_count(&conn, "2025-01-01").unwrap(), 2);
        soft_delete_schedule(&conn, &id2).unwrap();
        assert_eq!(overdue_count(&conn, "2025-01-01").unwrap(), 1);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        soft_delete_schedule(&conn, &id).unwrap();
        assert!(get_schedule(&conn, &id).unwrap().is_none());
        restore_schedule(&conn, &id).unwrap();
        assert!(get_schedule(&conn, &id).unwrap().is_some());
    }

    #[test]
    fn permanent_delete_only_removes_trashed() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        permanent_delete_schedule(&conn, &id).unwrap();
        assert!(get_schedule(&conn, &id).unwrap().is_some(), "active row survives permanent_delete");
        soft_delete_schedule(&conn, &id).unwrap();
        permanent_delete_schedule(&conn, &id).unwrap();
        // Verify the row is gone (even including trashed).
        let row: Option<i64> = conn.query_row(
            "SELECT 1 FROM maintenance_schedule WHERE id = ?1", params![id], |r| r.get(0),
        ).optional().unwrap();
        assert!(row.is_none());
    }
}
