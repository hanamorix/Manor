//! Maintenance event DAL (L4c).

use super::event::{EventSource, EventWithContext, MaintenanceEvent, MaintenanceEventDraft};
use crate::ledger::transaction::Transaction;
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

pub fn asset_spend_totals(
    conn: &Connection,
    today: &str,
) -> Result<Vec<crate::maintenance::event::AssetSpendTotal>> {
    let mut stmt = conn.prepare(
        "WITH cutoff AS (SELECT date(?1, '-365 days') AS d365)
         SELECT
             a.id           AS asset_id,
             a.name         AS asset_name,
             a.category     AS asset_category,
             COALESCE(SUM(CASE
                 WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                  AND e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_last_12m_pence,
             COALESCE(SUM(CASE
                 WHEN e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_lifetime_pence,
             COALESCE(SUM(CASE
                 WHEN e.id IS NOT NULL
                  AND e.completed_date >= (SELECT d365 FROM cutoff)
                  AND e.deleted_at IS NULL
                 THEN 1 END), 0) AS event_count_last_12m,
             COALESCE(SUM(CASE
                 WHEN e.id IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN 1 END), 0) AS event_count_lifetime
         FROM asset a
         LEFT JOIN maintenance_event e ON e.asset_id = a.id
         WHERE a.deleted_at IS NULL
         GROUP BY a.id
         ORDER BY total_last_12m_pence DESC, a.name COLLATE NOCASE ASC",
    )?;
    let rows = stmt
        .query_map(params![today], |row| {
            Ok(crate::maintenance::event::AssetSpendTotal {
                asset_id: row.get("asset_id")?,
                asset_name: row.get("asset_name")?,
                asset_category: row.get("asset_category")?,
                total_last_12m_pence: row.get("total_last_12m_pence")?,
                total_lifetime_pence: row.get("total_lifetime_pence")?,
                event_count_last_12m: row.get("event_count_last_12m")?,
                event_count_lifetime: row.get("event_count_lifetime")?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn asset_spend_for_asset(
    conn: &Connection,
    asset_id: &str,
    today: &str,
) -> Result<crate::maintenance::event::AssetSpendTotal> {
    let totals = asset_spend_totals(conn, today)?;
    totals
        .into_iter()
        .find(|r| r.asset_id == asset_id)
        .ok_or_else(|| anyhow!("Asset not found or trashed"))
}

pub fn category_spend_totals(
    conn: &Connection,
    today: &str,
) -> Result<Vec<crate::maintenance::event::CategorySpendTotal>> {
    let mut stmt = conn.prepare(
        "WITH cutoff AS (SELECT date(?1, '-365 days') AS d365)
         SELECT
             a.category AS category,
             COALESCE(SUM(CASE
                 WHEN e.completed_date >= (SELECT d365 FROM cutoff)
                  AND e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_last_12m_pence,
             COALESCE(SUM(CASE
                 WHEN e.cost_pence IS NOT NULL
                  AND e.deleted_at IS NULL
                 THEN e.cost_pence END), 0) AS total_lifetime_pence
         FROM asset a
         LEFT JOIN maintenance_event e ON e.asset_id = a.id
         WHERE a.deleted_at IS NULL
         GROUP BY a.category",
    )?;
    let rows = stmt
        .query_map(params![today], |row| {
            Ok(crate::maintenance::event::CategorySpendTotal {
                category: row.get("category")?,
                total_last_12m_pence: row.get("total_last_12m_pence")?,
                total_lifetime_pence: row.get("total_lifetime_pence")?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
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

fn tx_from_row(row: &Row) -> rusqlite::Result<Transaction> {
    Ok(Transaction {
        id: row.get("id")?,
        bank_account_id: row.get("bank_account_id")?,
        amount_pence: row.get("amount_pence")?,
        currency: row.get("currency")?,
        description: row.get("description")?,
        merchant: row.get("merchant")?,
        category_id: row.get("category_id")?,
        date: row.get("date")?,
        source: row.get("source")?,
        note: row.get("note")?,
        recurring_payment_id: row.get("recurring_payment_id")?,
        created_at: row.get("created_at")?,
    })
}

pub fn suggest_transactions(
    conn: &Connection,
    completed_date: &str,
    cost_pence: Option<i64>,
    exclude_event_id: Option<&str>,
    limit: usize,
) -> Result<Vec<Transaction>> {
    let exclude = exclude_event_id.unwrap_or("");
    let lim = limit as i64;
    let rows = match cost_pence {
        Some(c) => {
            let mut stmt = conn.prepare(
                "SELECT lt.id, lt.bank_account_id, lt.amount_pence, lt.currency,
                        lt.description, lt.merchant, lt.category_id, lt.date,
                        lt.source, lt.note, lt.recurring_payment_id, lt.created_at
                 FROM ledger_transaction lt
                 LEFT JOIN maintenance_event me
                     ON me.transaction_id = lt.id AND me.deleted_at IS NULL
                 WHERE lt.deleted_at IS NULL
                   AND (me.id IS NULL OR me.id = ?1)
                   AND date(lt.date, 'unixepoch') BETWEEN date(?2, '-7 days')
                                                      AND date(?2, '+2 days')
                 ORDER BY ABS(lt.amount_pence + ?3) ASC
                 LIMIT ?4",
            )?;
            let v = stmt
                .query_map(params![exclude, completed_date, c, lim], tx_from_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            v
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT lt.id, lt.bank_account_id, lt.amount_pence, lt.currency,
                        lt.description, lt.merchant, lt.category_id, lt.date,
                        lt.source, lt.note, lt.recurring_payment_id, lt.created_at
                 FROM ledger_transaction lt
                 LEFT JOIN maintenance_event me
                     ON me.transaction_id = lt.id AND me.deleted_at IS NULL
                 WHERE lt.deleted_at IS NULL
                   AND (me.id IS NULL OR me.id = ?1)
                   AND date(lt.date, 'unixepoch') BETWEEN date(?2, '-7 days')
                                                      AND date(?2, '+2 days')
                 ORDER BY lt.date DESC
                 LIMIT ?3",
            )?;
            let v = stmt
                .query_map(params![exclude, completed_date, lim], tx_from_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            v
        }
    };
    Ok(rows)
}

pub fn search_transactions(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<Transaction>> {
    let like = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT lt.id, lt.bank_account_id, lt.amount_pence, lt.currency,
                lt.description, lt.merchant, lt.category_id, lt.date,
                lt.source, lt.note, lt.recurring_payment_id, lt.created_at
         FROM ledger_transaction lt
         LEFT JOIN maintenance_event me
             ON me.transaction_id = lt.id AND me.deleted_at IS NULL
         WHERE lt.deleted_at IS NULL
           AND me.id IS NULL
           AND (lt.description LIKE ?1 COLLATE NOCASE OR lt.merchant LIKE ?1 COLLATE NOCASE)
         ORDER BY lt.date DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![like, limit as i64], tx_from_row)?
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

    fn insert_asset_with_category(
        conn: &Connection,
        name: &str,
        category: AssetCategory,
    ) -> String {
        let asset = AssetDraft {
            name: name.into(),
            category,
            make: None,
            model: None,
            serial_number: None,
            purchase_date: None,
            notes: String::new(),
            hero_attachment_uuid: None,
        };
        asset_dal::insert_asset(conn, &asset).unwrap()
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

    #[test]
    fn asset_spend_totals_zero_events_shows_asset_with_zeros() {
        let (_d, conn, asset_id) = fresh();
        let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].asset_id, asset_id);
        assert_eq!(rows[0].total_last_12m_pence, 0);
        assert_eq!(rows[0].event_count_lifetime, 0);
    }

    #[test]
    fn asset_spend_totals_12m_window() {
        let (_d, conn, asset_id) = fresh();
        let mut d = draft(&asset_id);
        d.completed_date = "2025-04-21".into(); // 364 days before 2026-04-20 — inside
        d.cost_pence = Some(10000);
        insert_event(&conn, &d).unwrap();
        d.completed_date = "2025-04-19".into(); // 366 days before — outside
        d.cost_pence = Some(50000);
        insert_event(&conn, &d).unwrap();
        let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
        let row = rows.iter().find(|r| r.asset_id == asset_id).unwrap();
        assert_eq!(row.total_last_12m_pence, 10000);
        assert_eq!(row.total_lifetime_pence, 60000);
        assert_eq!(row.event_count_last_12m, 1);
        assert_eq!(row.event_count_lifetime, 2);
    }

    #[test]
    fn asset_spend_totals_null_cost_counts_but_not_sum() {
        let (_d, conn, asset_id) = fresh();
        let mut d = draft(&asset_id);
        d.cost_pence = None;
        d.completed_date = "2026-01-10".into();
        insert_event(&conn, &d).unwrap();
        let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
        let row = rows.iter().find(|r| r.asset_id == asset_id).unwrap();
        assert_eq!(row.total_lifetime_pence, 0);
        assert_eq!(row.event_count_lifetime, 1);
    }

    #[test]
    fn asset_spend_totals_excludes_trashed_assets() {
        let (_d, conn, asset_id) = fresh();
        conn.execute(
            "UPDATE asset SET deleted_at = 1 WHERE id = ?1",
            params![asset_id],
        )
        .unwrap();
        let rows = asset_spend_totals(&conn, "2026-04-20").unwrap();
        assert!(rows.iter().all(|r| r.asset_id != asset_id));
    }

    #[test]
    fn category_spend_totals_sums_by_category() {
        let (_d, conn, _base_asset_id) = fresh();
        let appliance_id = insert_asset_with_category(&conn, "Boiler 2", AssetCategory::Appliance);
        let vehicle_id = insert_asset_with_category(&conn, "Car", AssetCategory::Vehicle);
        let mut d = draft(&appliance_id);
        d.cost_pence = Some(10000);
        insert_event(&conn, &d).unwrap();
        let mut d2 = draft(&vehicle_id);
        d2.cost_pence = Some(25000);
        insert_event(&conn, &d2).unwrap();
        let rows = category_spend_totals(&conn, "2026-04-20").unwrap();
        let appliance = rows.iter().find(|r| r.category == "appliance").unwrap();
        let vehicle = rows.iter().find(|r| r.category == "vehicle").unwrap();
        // fresh()'s base asset is Appliance with no events, so appliance total = 10000 from Boiler 2
        assert_eq!(appliance.total_lifetime_pence, 10000);
        assert_eq!(vehicle.total_lifetime_pence, 25000);
    }

    #[test]
    fn suggest_with_cost_ranks_by_amount_proximity() {
        let (_d, conn, _asset_id) = fresh();
        // Three transactions on 2026-04-20 (unix = 1776513600 = 2026-04-20 00:00 UTC)
        let date_ts = 1776513600i64;
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-10000, 'GBP', 'Tesco',       ?1, 'manual'),
                    (-14500, 'GBP', 'British Gas', ?1, 'manual'),
                    (-20000, 'GBP', 'Argos',       ?1, 'manual')",
            params![date_ts],
        )
        .unwrap();
        let rows = suggest_transactions(&conn, "2026-04-20", Some(14500), None, 3).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].description, "British Gas"); // closest match first
    }

    #[test]
    fn suggest_without_cost_orders_by_date_desc() {
        let (_d, conn, _asset_id) = fresh();
        let base_ts = 1776513600i64; // 2026-04-20
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-10000, 'GBP', 'Earlier', ?1, 'manual'),
                    (-10000, 'GBP', 'Later',   ?2, 'manual')",
            params![base_ts - 86400, base_ts], // Earlier = one day before, Later = same day
        )
        .unwrap();
        let rows = suggest_transactions(&conn, "2026-04-20", None, None, 5).unwrap();
        assert!(!rows.is_empty());
        assert_eq!(rows[0].description, "Later");
    }

    #[test]
    fn suggest_excludes_already_linked() {
        let (_d, conn, asset_id) = fresh();
        let date_ts = 1776513600i64;
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-14500, 'GBP', 'British Gas', ?1, 'manual')",
            params![date_ts],
        )
        .unwrap();
        let tx_id = conn.last_insert_rowid();
        let mut d = draft(&asset_id);
        d.transaction_id = Some(tx_id);
        insert_event(&conn, &d).unwrap();
        let rows = suggest_transactions(&conn, "2026-04-20", Some(14500), None, 3).unwrap();
        assert!(rows.iter().all(|t| t.id != tx_id));
    }

    #[test]
    fn suggest_includes_self_when_exclude_event_id_set() {
        let (_d, conn, asset_id) = fresh();
        let date_ts = 1776513600i64;
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, date, source)
             VALUES (-14500, 'GBP', 'British Gas', ?1, 'manual')",
            params![date_ts],
        )
        .unwrap();
        let tx_id = conn.last_insert_rowid();
        let mut d = draft(&asset_id);
        d.transaction_id = Some(tx_id);
        let event_id = insert_event(&conn, &d).unwrap();
        let rows =
            suggest_transactions(&conn, "2026-04-20", Some(14500), Some(&event_id), 3).unwrap();
        assert!(rows.iter().any(|t| t.id == tx_id));
    }

    #[test]
    fn search_matches_description_and_merchant() {
        let (_d, conn, _asset_id) = fresh();
        let date_ts = 1776513600i64;
        conn.execute(
            "INSERT INTO ledger_transaction (amount_pence, currency, description, merchant, date, source)
             VALUES (-14500, 'GBP', 'Boiler service', 'British Gas Ltd', ?1, 'manual')",
            params![date_ts],
        )
        .unwrap();
        let rows = search_transactions(&conn, "british", 10).unwrap();
        assert!(
            !rows.is_empty(),
            "search for 'british' should match merchant"
        );
        let rows2 = search_transactions(&conn, "boiler", 10).unwrap();
        assert!(
            !rows2.is_empty(),
            "search for 'boiler' should match description"
        );
    }
}
