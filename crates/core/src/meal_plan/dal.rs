//! Meal plan DAL: week/today/set/clear.

use super::MealPlanEntry;
use anyhow::Result;
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Return 7 entries starting at `start_date` (ISO YYYY-MM-DD, expected to be a Monday).
/// Dates without a persisted entry get a synthetic entry with recipe_id=None.
pub fn get_week(conn: &Connection, start_date: &str) -> Result<Vec<MealPlanEntry>> {
    let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")?;
    let mut out = Vec::with_capacity(7);
    for offset in 0..7 {
        let d = start + chrono::Duration::days(offset);
        let date_str = d.format("%Y-%m-%d").to_string();
        if let Some(entry) = get_entry(conn, &date_str)? {
            out.push(entry);
        } else {
            out.push(MealPlanEntry {
                id: String::new(),
                entry_date: date_str,
                recipe_id: None,
                created_at: 0,
                updated_at: 0,
            });
        }
    }
    Ok(out)
}

pub fn get_entry(conn: &Connection, date: &str) -> Result<Option<MealPlanEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, entry_date, recipe_id, created_at, updated_at
         FROM meal_plan_entry WHERE entry_date = ?1",
    )?;
    let row = stmt.query_row(params![date], |r| Ok(MealPlanEntry {
        id: r.get(0)?,
        entry_date: r.get(1)?,
        recipe_id: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
    })).optional()?;
    Ok(row)
}

pub fn set_entry(conn: &Connection, date: &str, recipe_id: &str) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "INSERT INTO meal_plan_entry (id, entry_date, recipe_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(entry_date) DO UPDATE
           SET recipe_id = excluded.recipe_id, updated_at = excluded.updated_at",
        params![Uuid::new_v4().to_string(), date, recipe_id, now],
    )?;
    Ok(())
}

pub fn clear_entry(conn: &Connection, date: &str) -> Result<()> {
    conn.execute("DELETE FROM meal_plan_entry WHERE entry_date = ?1", params![date])?;
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

    fn insert_recipe(conn: &Connection, title: &str) -> String {
        let draft = crate::recipe::RecipeDraft {
            title: title.into(),
            servings: None, prep_time_mins: None, cook_time_mins: None,
            instructions: "".into(),
            source_url: None, source_host: None,
            import_method: crate::recipe::ImportMethod::Manual,
            hero_attachment_uuid: None,
            ingredients: vec![],
        };
        crate::recipe::dal::insert_recipe(conn, &draft).unwrap()
    }

    #[test]
    fn get_week_returns_seven_entries_with_none_for_missing_dates() {
        let (_d, conn) = fresh();
        let rid = insert_recipe(&conn, "Miso");
        set_entry(&conn, "2026-04-22", &rid).unwrap();

        let week = get_week(&conn, "2026-04-20").unwrap();
        assert_eq!(week.len(), 7);
        assert_eq!(week[0].entry_date, "2026-04-20"); assert!(week[0].recipe_id.is_none());
        assert_eq!(week[2].entry_date, "2026-04-22"); assert_eq!(week[2].recipe_id.as_deref(), Some(rid.as_str()));
        assert_eq!(week[6].entry_date, "2026-04-26"); assert!(week[6].recipe_id.is_none());
    }

    #[test]
    fn set_entry_upserts_on_same_date() {
        let (_d, conn) = fresh();
        let a = insert_recipe(&conn, "A");
        let b = insert_recipe(&conn, "B");
        set_entry(&conn, "2026-04-22", &a).unwrap();
        set_entry(&conn, "2026-04-22", &b).unwrap();
        let week = get_week(&conn, "2026-04-20").unwrap();
        assert_eq!(week[2].recipe_id.as_deref(), Some(b.as_str()));
    }

    #[test]
    fn clear_entry_removes_row() {
        let (_d, conn) = fresh();
        let a = insert_recipe(&conn, "A");
        set_entry(&conn, "2026-04-22", &a).unwrap();
        clear_entry(&conn, "2026-04-22").unwrap();
        let week = get_week(&conn, "2026-04-20").unwrap();
        assert!(week[2].recipe_id.is_none());
    }

    #[test]
    fn get_entry_returns_none_when_absent() {
        let (_d, conn) = fresh();
        assert!(get_entry(&conn, "2026-04-22").unwrap().is_none());
    }
}
