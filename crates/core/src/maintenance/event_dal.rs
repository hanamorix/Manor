//! Maintenance event DAL (L4c).

use super::event::{EventSource, EventWithContext, MaintenanceEvent, MaintenanceEventDraft};
use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn insert_event(conn: &Connection, draft: &MaintenanceEventDraft) -> Result<String> {
    validate_draft(conn, draft)?;
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    conn.execute(
        "INSERT INTO maintenance_event
           (id, asset_id, schedule_id, title, completed_date, cost_pence, currency,
            notes, transaction_id, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'manual', ?10, ?11)",
        params![
            id,
            draft.asset_id,
            draft.schedule_id,
            draft.title,
            draft.completed_date,
            draft.cost_pence,
            draft.currency,
            draft.notes,
            draft.transaction_id,
            now,
            now,
        ],
    )
    .map_err(translate_constraint_err)?;
    Ok(id)
}

pub fn get_event(conn: &Connection, id: &str) -> Result<Option<MaintenanceEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, asset_id, schedule_id, title, completed_date, cost_pence, currency,
                notes, transaction_id, source, created_at, updated_at, deleted_at
         FROM maintenance_event WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_event(row)?))
    } else {
        Ok(None)
    }
}

pub fn update_event(conn: &Connection, id: &str, draft: &MaintenanceEventDraft) -> Result<()> {
    validate_draft(conn, draft)?;
    let now = now_secs();
    let changed = conn
        .execute(
            "UPDATE maintenance_event
               SET title = ?1, completed_date = ?2, cost_pence = ?3, currency = ?4,
                   notes = ?5, transaction_id = ?6, updated_at = ?7
             WHERE id = ?8 AND deleted_at IS NULL",
            params![
                draft.title,
                draft.completed_date,
                draft.cost_pence,
                draft.currency,
                draft.notes,
                draft.transaction_id,
                now,
                id,
            ],
        )
        .map_err(translate_constraint_err)?;
    if changed == 0 {
        return Err(anyhow!("Event not found or already deleted"));
    }
    Ok(())
}

fn validate_draft(conn: &Connection, draft: &MaintenanceEventDraft) -> Result<()> {
    if let Some(c) = draft.cost_pence {
        if c < 0 {
            return Err(anyhow!("Cost must be zero or positive"));
        }
    }
    NaiveDate::parse_from_str(&draft.completed_date, "%Y-%m-%d")
        .map_err(|_| anyhow!("Date must be in YYYY-MM-DD format"))?;
    if let Some(sched_id) = &draft.schedule_id {
        let owner: Option<String> = conn
            .query_row(
                "SELECT asset_id FROM maintenance_schedule WHERE id = ?1",
                params![sched_id],
                |r| r.get(0),
            )
            .ok();
        match owner {
            Some(aid) if aid == draft.asset_id => {}
            Some(_) => return Err(anyhow!("Schedule does not belong to asset")),
            None => return Err(anyhow!("Schedule not found")),
        }
    }
    Ok(())
}

fn translate_constraint_err(err: rusqlite::Error) -> anyhow::Error {
    let s = err.to_string();
    // SQLite partial unique-index violation surfaces as:
    //   "UNIQUE constraint failed: maintenance_event.transaction_id"
    if s.contains("maintenance_event.transaction_id") || s.contains("idx_evt_tx_unique") {
        anyhow!("Transaction already linked to another event")
    } else {
        anyhow!(err)
    }
}

fn row_to_event(row: &Row) -> Result<MaintenanceEvent> {
    let source_str: String = row.get("source")?;
    Ok(MaintenanceEvent {
        id: row.get("id")?,
        asset_id: row.get("asset_id")?,
        schedule_id: row.get("schedule_id")?,
        title: row.get("title")?,
        completed_date: row.get("completed_date")?,
        cost_pence: row.get("cost_pence")?,
        currency: row.get("currency")?,
        notes: row.get("notes")?,
        transaction_id: row.get("transaction_id")?,
        source: EventSource::parse(&source_str)?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

pub fn list_for_asset(conn: &Connection, asset_id: &str) -> Result<Vec<EventWithContext>> {
    let mut stmt = conn.prepare(
        "SELECT
             me.id, me.asset_id, me.schedule_id, me.title, me.completed_date,
             me.cost_pence, me.currency, me.notes, me.transaction_id, me.source,
             me.created_at, me.updated_at, me.deleted_at,
             ms.task AS schedule_task,
             CASE WHEN ms.deleted_at IS NOT NULL THEN 1 ELSE 0 END AS schedule_deleted_flag,
             lt.description AS tx_description,
             lt.amount_pence AS tx_amount,
             lt.date AS tx_date
         FROM maintenance_event me
         LEFT JOIN maintenance_schedule ms ON ms.id = me.schedule_id
         LEFT JOIN ledger_transaction lt
             ON lt.id = me.transaction_id AND lt.deleted_at IS NULL
         WHERE me.asset_id = ?1 AND me.deleted_at IS NULL
         ORDER BY me.completed_date DESC, me.created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![asset_id], |row| {
            let source_str: String = row.get("source")?;
            let source = EventSource::parse(&source_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )),
                )
            })?;
            let event = MaintenanceEvent {
                id: row.get("id")?,
                asset_id: row.get("asset_id")?,
                schedule_id: row.get("schedule_id")?,
                title: row.get("title")?,
                completed_date: row.get("completed_date")?,
                cost_pence: row.get("cost_pence")?,
                currency: row.get("currency")?,
                notes: row.get("notes")?,
                transaction_id: row.get("transaction_id")?,
                source,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
                deleted_at: row.get("deleted_at")?,
            };
            let schedule_deleted_flag: i64 = row.get("schedule_deleted_flag")?;
            Ok(EventWithContext {
                event,
                schedule_task: row.get("schedule_task")?,
                schedule_deleted: schedule_deleted_flag != 0,
                transaction_description: row.get("tx_description")?,
                transaction_amount_pence: row.get("tx_amount")?,
                transaction_date: row.get("tx_date")?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{dal as asset_dal, AssetCategory, AssetDraft};
    use crate::assistant::db;
    use crate::maintenance::dal as sched_dal;
    use crate::maintenance::MaintenanceScheduleDraft;
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

    fn insert_test_schedule(
        conn: &Connection,
        asset_id: &str,
        task: &str,
        interval_months: i32,
    ) -> String {
        let draft = MaintenanceScheduleDraft {
            asset_id: asset_id.into(),
            task: task.into(),
            interval_months,
            last_done_date: None,
            notes: String::new(),
        };
        sched_dal::insert_schedule(conn, &draft).unwrap()
    }

    fn draft(asset_id: &str) -> MaintenanceEventDraft {
        MaintenanceEventDraft {
            asset_id: asset_id.to_string(),
            schedule_id: None,
            title: "Annual boiler service".into(),
            completed_date: "2026-04-20".into(),
            cost_pence: Some(14500),
            currency: "GBP".into(),
            notes: "".into(),
            transaction_id: None,
        }
    }

    #[test]
    fn insert_and_get_round_trip() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_event(&conn, &draft(&asset_id)).unwrap();
        let got = get_event(&conn, &id).unwrap().unwrap();
        assert_eq!(got.asset_id, asset_id);
        assert_eq!(got.title, "Annual boiler service");
        assert_eq!(got.cost_pence, Some(14500));
        assert_eq!(got.source, EventSource::Manual);
    }

    #[test]
    fn insert_rejects_negative_cost() {
        let (_d, conn, asset_id) = fresh();
        let mut d = draft(&asset_id);
        d.cost_pence = Some(-100);
        let err = insert_event(&conn, &d).unwrap_err().to_string();
        assert!(err.contains("zero or positive"), "got: {}", err);
    }

    #[test]
    fn insert_rejects_bad_date() {
        let (_d, conn, asset_id) = fresh();
        let mut d = draft(&asset_id);
        d.completed_date = "not-a-date".into();
        let err = insert_event(&conn, &d).unwrap_err().to_string();
        assert!(err.contains("YYYY-MM-DD"), "got: {}", err);
    }

    #[test]
    fn insert_rejects_schedule_asset_mismatch() {
        let (_d, conn, asset_a) = fresh();
        let asset_b_draft = AssetDraft {
            name: "Asset B".into(),
            category: AssetCategory::Appliance,
            make: None,
            model: None,
            serial_number: None,
            purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        let asset_b = asset_dal::insert_asset(&conn, &asset_b_draft).unwrap();
        let sched_a = insert_test_schedule(&conn, &asset_a, "task", 12);
        let mut d = draft(&asset_b);
        d.schedule_id = Some(sched_a);
        let err = insert_event(&conn, &d).unwrap_err().to_string();
        assert!(err.contains("does not belong"), "got: {}", err);
    }

    #[test]
    fn update_preserves_source() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_event(&conn, &draft(&asset_id)).unwrap();
        let mut d = draft(&asset_id);
        d.cost_pence = Some(20000);
        d.notes = "£200 service".into();
        update_event(&conn, &id, &d).unwrap();
        let got = get_event(&conn, &id).unwrap().unwrap();
        assert_eq!(got.cost_pence, Some(20000));
        assert_eq!(got.notes, "£200 service");
        assert_eq!(got.source, EventSource::Manual);
    }

    #[test]
    fn update_can_clear_transaction() {
        let (_d, conn, asset_id) = fresh();
        let id = insert_event(&conn, &draft(&asset_id)).unwrap();
        let mut d2 = draft(&asset_id);
        d2.transaction_id = None;
        update_event(&conn, &id, &d2).unwrap();
        let got = get_event(&conn, &id).unwrap().unwrap();
        assert_eq!(got.transaction_id, None);
    }

    #[test]
    fn list_for_asset_orders_desc_and_populates_context() {
        let (_d, conn, asset_id) = fresh();
        let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
        let mut d = draft(&asset_id);
        d.schedule_id = Some(sched_id.clone());
        d.completed_date = "2025-01-10".into();
        insert_event(&conn, &d).unwrap();
        d.completed_date = "2026-02-20".into();
        insert_event(&conn, &d).unwrap();
        let rows = list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].event.completed_date, "2026-02-20");
        assert_eq!(rows[1].event.completed_date, "2025-01-10");
        assert_eq!(rows[0].schedule_task.as_deref(), Some("Service"));
        assert!(!rows[0].schedule_deleted);
    }

    #[test]
    fn list_for_asset_marks_schedule_deleted() {
        let (_d, conn, asset_id) = fresh();
        let sched_id = insert_test_schedule(&conn, &asset_id, "Service", 12);
        let mut d = draft(&asset_id);
        d.schedule_id = Some(sched_id.clone());
        insert_event(&conn, &d).unwrap();
        // soft-delete the schedule
        conn.execute(
            "UPDATE maintenance_schedule SET deleted_at = 1 WHERE id = ?1",
            params![sched_id],
        )
        .unwrap();
        let rows = list_for_asset(&conn, &asset_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].schedule_deleted);
        assert_eq!(rows[0].schedule_task.as_deref(), Some("Service")); // still resolvable
    }

    #[test]
    fn transaction_unique_index_rejects_duplicate_link() {
        let (_d, conn, asset_id) = fresh();
        // Insert a real ledger_transaction row first so FK holds.
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-14500, 'GBP', 'British Gas service', 1713628800, 'manual')",
            [],
        )
        .unwrap();
        let tx_id = conn.last_insert_rowid();

        let mut d1 = draft(&asset_id);
        d1.transaction_id = Some(tx_id);
        insert_event(&conn, &d1).unwrap();

        let mut d2 = draft(&asset_id);
        d2.transaction_id = Some(tx_id);
        let err = insert_event(&conn, &d2).unwrap_err().to_string();
        assert!(err.contains("already linked"), "got: {}", err);
    }

    #[test]
    fn transaction_link_re_allowed_after_soft_delete() {
        let (_d, conn, asset_id) = fresh();
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-14500, 'GBP', 'British Gas', 1713628800, 'manual')",
            [],
        )
        .unwrap();
        let tx_id = conn.last_insert_rowid();

        let mut d1 = draft(&asset_id);
        d1.transaction_id = Some(tx_id);
        let id1 = insert_event(&conn, &d1).unwrap();
        // soft-delete event 1
        conn.execute(
            "UPDATE maintenance_event SET deleted_at = 1 WHERE id = ?1",
            params![id1],
        )
        .unwrap();

        let mut d2 = draft(&asset_id);
        d2.transaction_id = Some(tx_id);
        insert_event(&conn, &d2).unwrap(); // should succeed
    }
}
