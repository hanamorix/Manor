//! Shopping list DAL: list/insert/toggle/delete/wipe.

use super::{ItemSource, ShoppingListItem};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_secs() -> i64 { chrono::Utc::now().timestamp() }

pub fn list_items(conn: &Connection) -> Result<Vec<ShoppingListItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, ingredient_name, quantity_text, note, recipe_id, recipe_title,
                source, position, ticked, created_at, updated_at
         FROM shopping_list_item
         ORDER BY ticked ASC, position ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        let source_s: String = r.get(6)?;
        let ticked_i: i64 = r.get(8)?;
        Ok(ShoppingListItem {
            id: r.get(0)?,
            ingredient_name: r.get(1)?,
            quantity_text: r.get(2)?,
            note: r.get(3)?,
            recipe_id: r.get(4)?,
            recipe_title: r.get(5)?,
            source: ItemSource::from_db(Some(source_s.as_str())),
            position: r.get(7)?,
            ticked: ticked_i != 0,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn insert_manual(conn: &Connection, ingredient_name: &str) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    let pos = next_position(conn)?;
    conn.execute(
        "INSERT INTO shopping_list_item
           (id, ingredient_name, quantity_text, note, recipe_id, recipe_title,
            source, position, ticked, created_at, updated_at)
         VALUES (?1, ?2, NULL, NULL, NULL, NULL, 'manual', ?3, 0, ?4, ?4)",
        params![id, ingredient_name, pos, now],
    )?;
    Ok(id)
}

pub fn toggle_tick(conn: &Connection, id: &str) -> Result<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE shopping_list_item SET ticked = 1 - ticked, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn delete_item(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM shopping_list_item WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn wipe_generated(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM shopping_list_item WHERE source = 'generated'", [])?;
    Ok(())
}

pub fn counts(conn: &Connection) -> Result<(usize, usize)> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM shopping_list_item", [], |r| r.get(0),
    )?;
    let ticked: i64 = conn.query_row(
        "SELECT COUNT(*) FROM shopping_list_item WHERE ticked = 1", [], |r| r.get(0),
    )?;
    Ok((total as usize, ticked as usize))
}

pub(crate) fn next_position(conn: &Connection) -> Result<i64> {
    let max: Option<i64> = conn.query_row(
        "SELECT MAX(position) FROM shopping_list_item", [], |r| r.get(0),
    ).optional()?.flatten();
    Ok(max.map(|m| m + 1).unwrap_or(0))
}

/// Insert a generated row. Called by the generator only.
pub(crate) fn insert_generated(
    conn: &Connection,
    ingredient_name: &str,
    quantity_text: Option<&str>,
    note: Option<&str>,
    recipe_id: &str,
    recipe_title: &str,
    position: i64,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_secs();
    conn.execute(
        "INSERT INTO shopping_list_item
           (id, ingredient_name, quantity_text, note, recipe_id, recipe_title,
            source, position, ticked, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'generated', ?7, 0, ?8, ?8)",
        params![id, ingredient_name, quantity_text, note, recipe_id, recipe_title, position, now],
    )?;
    Ok(id)
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
    fn insert_manual_roundtrips_with_expected_defaults() {
        let (_d, conn) = fresh();
        let id = insert_manual(&conn, "bin bags").unwrap();
        let list = list_items(&conn).unwrap();
        assert_eq!(list.len(), 1);
        let item = &list[0];
        assert_eq!(item.id, id);
        assert_eq!(item.ingredient_name, "bin bags");
        assert!(item.quantity_text.is_none());
        assert!(item.recipe_id.is_none());
        assert_eq!(item.source, ItemSource::Manual);
        assert!(!item.ticked);
    }

    #[test]
    fn manual_positions_land_after_all_existing_rows() {
        let (_d, conn) = fresh();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        insert_generated(&conn, "salt", None, None, "r1", "Recipe 1", 0).unwrap();
        insert_generated(&conn, "garlic", None, None, "r1", "Recipe 1", 1).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let manual_id = insert_manual(&conn, "bin bags").unwrap();
        let list = list_items(&conn).unwrap();
        // Expect order: salt (pos 0), garlic (pos 1), bin bags (pos 2).
        assert_eq!(list[2].id, manual_id);
        assert_eq!(list[2].position, 2);
    }

    #[test]
    fn list_orders_ticked_to_bottom_preserving_position() {
        let (_d, conn) = fresh();
        let a = insert_manual(&conn, "A").unwrap();
        let b = insert_manual(&conn, "B").unwrap();
        let c = insert_manual(&conn, "C").unwrap();
        toggle_tick(&conn, &b).unwrap();
        let list = list_items(&conn).unwrap();
        let names: Vec<_> = list.iter().map(|i| i.ingredient_name.as_str()).collect();
        assert_eq!(names, vec!["A", "C", "B"]);
        let _ = (a, c);
    }

    #[test]
    fn toggle_tick_flips() {
        let (_d, conn) = fresh();
        let id = insert_manual(&conn, "milk").unwrap();
        toggle_tick(&conn, &id).unwrap();
        assert!(list_items(&conn).unwrap()[0].ticked);
        toggle_tick(&conn, &id).unwrap();
        assert!(!list_items(&conn).unwrap()[0].ticked);
    }

    #[test]
    fn delete_item_removes_row() {
        let (_d, conn) = fresh();
        let id = insert_manual(&conn, "A").unwrap();
        delete_item(&conn, &id).unwrap();
        assert!(list_items(&conn).unwrap().is_empty());
    }

    #[test]
    fn wipe_generated_leaves_manual_untouched() {
        let (_d, conn) = fresh();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        insert_generated(&conn, "salt", None, None, "r1", "R1", 0).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let manual = insert_manual(&conn, "bin bags").unwrap();
        wipe_generated(&conn).unwrap();
        let list = list_items(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, manual);
    }

    #[test]
    fn counts_total_and_ticked() {
        let (_d, conn) = fresh();
        let a = insert_manual(&conn, "A").unwrap();
        let _b = insert_manual(&conn, "B").unwrap();
        let _c = insert_manual(&conn, "C").unwrap();
        toggle_tick(&conn, &a).unwrap();
        let (total, ticked) = counts(&conn).unwrap();
        assert_eq!(total, 3);
        assert_eq!(ticked, 1);
    }
}
