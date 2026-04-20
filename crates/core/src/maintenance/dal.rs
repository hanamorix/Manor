//! Maintenance schedule DAL: CRUD, mark_done, and band-query helpers.

use super::{due, MaintenanceSchedule, MaintenanceScheduleDraft};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn today_local() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

pub fn insert_schedule(conn: &Connection, draft: &MaintenanceScheduleDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let today = today_local();
    let next_due = due::compute_next_due(
        draft.last_done_date.as_deref(),
        draft.interval_months,
        &today,
    )?;
    conn.execute(
        "INSERT INTO maintenance_schedule
           (id, asset_id, task, interval_months, last_done_date, next_due_date, notes,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id,
            draft.asset_id,
            draft.task,
            draft.interval_months,
            draft.last_done_date,
            next_due,
            draft.notes,
            now,
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
    stmt.query_row(params![id], row_to_schedule)
        .optional()
        .map_err(Into::into)
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
    for row in rows {
        out.push(row?);
    }
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
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_due_today_and_overdue(
    conn: &Connection,
    today: &str,
) -> Result<Vec<MaintenanceSchedule>> {
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

pub fn update_schedule(
    conn: &Connection,
    id: &str,
    draft: &MaintenanceScheduleDraft,
) -> Result<()> {
    let now = now_secs();

    // Preserve the original creation date as fallback_start when last_done is None,
    // so editing the interval doesn't silently re-anchor next_due_date to "today".
    let created_at_secs: i64 = conn.query_row(
        "SELECT created_at FROM maintenance_schedule WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    let fallback = chrono::DateTime::<chrono::Utc>::from_timestamp(created_at_secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid created_at timestamp {}", created_at_secs))?
        .with_timezone(&chrono::Local)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    let next_due = due::compute_next_due(
        draft.last_done_date.as_deref(),
        draft.interval_months,
        &fallback,
    )?;
    conn.execute(
        "UPDATE maintenance_schedule
         SET asset_id = ?1, task = ?2, interval_months = ?3,
             last_done_date = ?4, next_due_date = ?5, notes = ?6, updated_at = ?7
         WHERE id = ?8",
        params![
            draft.asset_id,
            draft.task,
            draft.interval_months,
            draft.last_done_date,
            next_due,
            draft.notes,
            now,
            id,
        ],
    )?;
    Ok(())
}

pub fn mark_done(
    conn: &Connection,
    schedule_id: &str,
    today: &str,
    event_draft: Option<&crate::maintenance::event::MaintenanceEventDraft>,
) -> Result<String> {
    // Load schedule (get_schedule already excludes soft-deleted rows via AND deleted_at IS NULL)
    let schedule =
        get_schedule(conn, schedule_id)?.ok_or_else(|| anyhow::anyhow!("Schedule not found"))?;

    // Compute next due date
    let next_due = due::compute_next_due(Some(today), schedule.interval_months, today)?;
    let now = now_secs();

    // Bump schedule dates
    conn.execute(
        "UPDATE maintenance_schedule
         SET last_done_date = ?1, next_due_date = ?2, updated_at = ?3
         WHERE id = ?4",
        params![today, next_due, now, schedule_id],
    )?;

    // Build or use the provided event draft
    let minimal: crate::maintenance::event::MaintenanceEventDraft;
    let draft = match event_draft {
        Some(d) => d,
        None => {
            minimal = crate::maintenance::event::MaintenanceEventDraft {
                asset_id: schedule.asset_id.clone(),
                schedule_id: Some(schedule.id.clone()),
                title: schedule.task.clone(),
                completed_date: today.to_string(),
                cost_pence: None,
                currency: "GBP".to_string(),
                notes: String::new(),
                transaction_id: None,
            };
            &minimal
        }
    };

    // Insert the event and return its id
    let event_id = crate::maintenance::event_dal::insert_event(conn, draft)?;
    Ok(event_id)
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
        mark_done(&conn, &id, "2025-06-15", None).unwrap();
        let s = get_schedule(&conn, &id).unwrap().unwrap();
        assert_eq!(s.last_done_date.as_deref(), Some("2025-06-15"));
        assert_eq!(s.next_due_date, "2026-06-15");
    }

    #[test]
    fn mark_done_silent_inserts_minimal_event() {
        use crate::maintenance::event_dal;
        let (_d, conn, asset_id) = fresh();
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        let event_id = mark_done(&conn, &id, "2026-04-20", None).unwrap();
        let events = event_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(events.len(), 1);
        let e = &events[0].event;
        assert_eq!(e.id, event_id);
        assert_eq!(e.title, "Annual service");
        assert_eq!(e.completed_date, "2026-04-20");
        assert_eq!(e.cost_pence, None);
        assert_eq!(e.notes, "");
        assert_eq!(e.transaction_id, None);
    }

    #[test]
    fn mark_done_with_draft_uses_caller_draft() {
        use crate::maintenance::event::MaintenanceEventDraft;
        use crate::maintenance::event_dal;
        let (_d, conn, asset_id) = fresh();
        let sched_id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        let draft = MaintenanceEventDraft {
            asset_id: asset_id.clone(),
            schedule_id: Some(sched_id.clone()),
            title: "Annual service — upgraded parts".into(),
            completed_date: "2026-04-20".into(),
            cost_pence: Some(18000),
            currency: "GBP".into(),
            notes: "Replaced pump".into(),
            transaction_id: None,
        };
        mark_done(&conn, &sched_id, "2026-04-20", Some(&draft)).unwrap();
        let events = event_dal::list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(events[0].event.cost_pence, Some(18000));
        assert_eq!(events[0].event.notes, "Replaced pump");
    }

    #[test]
    fn mark_done_still_bumps_schedule_dates() {
        let (_d, conn, asset_id) = fresh();
        let sched_id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        mark_done(&conn, &sched_id, "2026-04-20", None).unwrap();
        let sched = get_schedule(&conn, &sched_id).unwrap().unwrap();
        assert_eq!(sched.last_done_date.as_deref(), Some("2026-04-20"));
        assert_eq!(sched.next_due_date, "2027-04-20");
    }

    #[test]
    fn list_for_asset_excludes_trashed_and_orders_by_due() {
        let (_d, conn, asset_id) = fresh();
        let a = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task A".into();
            d.last_done_date = Some("2024-06-15".into()); // next_due 2025-06-15
            d
        })
        .unwrap();
        let b = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task B".into();
            d.last_done_date = Some("2024-01-15".into()); // next_due 2025-01-15
            d
        })
        .unwrap();
        let c = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Task C".into();
            d.last_done_date = Some("2024-03-15".into()); // next_due 2025-03-15
            d
        })
        .unwrap();
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
            d.last_done_date = Some("2023-06-15".into()); // next_due 2024-06-15
            d
        })
        .unwrap();
        insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.task = "Future".into();
            d.last_done_date = Some("2026-01-15".into()); // next_due 2027-01-15
            d
        })
        .unwrap();

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
        })
        .unwrap();
        let id2 = insert_schedule(&conn, &{
            let mut d = simple_draft(&asset_id);
            d.last_done_date = Some("2023-07-15".into());
            d
        })
        .unwrap();
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
        assert!(
            get_schedule(&conn, &id).unwrap().is_some(),
            "active row survives permanent_delete"
        );
        soft_delete_schedule(&conn, &id).unwrap();
        permanent_delete_schedule(&conn, &id).unwrap();
        // Verify the row is gone (even including trashed).
        let row: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM maintenance_schedule WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert!(row.is_none());
    }

    #[test]
    fn update_never_done_preserves_created_at_anchor() {
        let (_d, conn, asset_id) = fresh();
        // Insert a never-done schedule. Its next_due_date = today + 12 months.
        let id = insert_schedule(&conn, &simple_draft(&asset_id)).unwrap();
        let original = get_schedule(&conn, &id).unwrap().unwrap();
        let original_next_due = original.next_due_date.clone();

        // Age the row's created_at to 100 days ago so the fallback differs from today.
        let aged = chrono::Local::now().timestamp() - 100 * 24 * 60 * 60;
        conn.execute(
            "UPDATE maintenance_schedule SET created_at = ?1 WHERE id = ?2",
            params![aged, id],
        )
        .unwrap();

        // Change interval; last_done stays None.
        let mut draft = simple_draft(&asset_id);
        draft.interval_months = 24;
        update_schedule(&conn, &id, &draft).unwrap();

        let updated = get_schedule(&conn, &id).unwrap().unwrap();

        // updated.next_due_date should be (created_at ≈ 100 days ago) + 24 months,
        // NOT "today + 24 months". If the fix works, those two differ by ~100 days.
        let today = chrono::Local::now().date_naive();
        let today_plus_24mo = today
            .checked_add_months(chrono::Months::new(24))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        assert_ne!(
            updated.next_due_date, today_plus_24mo,
            "update_schedule should anchor on created_at, not today"
        );
        let _ = original_next_due;
    }
}
