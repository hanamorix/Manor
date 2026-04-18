//! Shopping list regenerator — pure function.

use super::dal;
use super::GeneratedReport;
use anyhow::Result;
use rusqlite::Connection;

/// Regenerate the shopping list from the planned week starting at `week_start` (ISO YYYY-MM-DD).
/// Wipes rows where source='generated'; leaves manual rows. Appends newly-generated items
/// after existing manual rows in position space.
pub fn regenerate_from_week(conn: &Connection, week_start: &str) -> Result<GeneratedReport> {
    let staples = crate::meal_plan::staples::list_staples(conn)?;
    let entries = crate::meal_plan::dal::get_week(conn, week_start)?;

    let tx = conn.unchecked_transaction()?;

    dal::wipe_generated(&tx)?;
    let mut next_pos: i64 = dal::next_position(&tx)?;

    let mut report = GeneratedReport::default();

    for entry in entries {
        let Some(recipe_id) = entry.recipe_id else { continue };
        let Some(recipe) = crate::recipe::dal::get_recipe_including_trashed(&tx, &recipe_id)? else {
            continue;
        };
        if recipe.deleted_at.is_some() {
            report.ghost_recipes_skipped += 1;
            continue;
        }
        for ing in &recipe.ingredients {
            if crate::meal_plan::matcher::staple_matches(&ing.ingredient_name, &staples) {
                report.items_skipped_staple += 1;
                continue;
            }
            dal::insert_generated(
                &tx,
                &ing.ingredient_name,
                ing.quantity_text.as_deref(),
                ing.note.as_deref(),
                &recipe.id,
                &recipe.title,
                next_pos,
            )?;
            next_pos += 1;
            report.items_added += 1;
        }
    }

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use crate::recipe::{IngredientLine, ImportMethod, RecipeDraft};
    use crate::meal_plan::{StapleDraft};
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn insert_recipe_with(conn: &Connection, title: &str, ingredients: Vec<IngredientLine>) -> String {
        let draft = RecipeDraft {
            title: title.into(),
            servings: None, prep_time_mins: None, cook_time_mins: None,
            instructions: "".into(),
            source_url: None, source_host: None,
            import_method: ImportMethod::Manual,
            hero_attachment_uuid: None,
            ingredients,
        };
        crate::recipe::dal::insert_recipe(conn, &draft).unwrap()
    }

    fn line(name: &str) -> IngredientLine {
        IngredientLine { quantity_text: None, ingredient_name: name.into(), note: None }
    }

    #[test]
    fn happy_path_generates_minus_staples() {
        let (_d, conn) = fresh();
        let rid = insert_recipe_with(&conn, "Miso", vec![line("aubergine"), line("miso paste"), line("salt")]);
        crate::meal_plan::staples::insert_staple(&conn, &StapleDraft {
            name: "salt".into(), aliases: vec![],
        }).unwrap();
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.items_added, 2);
        assert_eq!(report.items_skipped_staple, 1);
        assert_eq!(report.ghost_recipes_skipped, 0);

        let items = dal::list_items(&conn).unwrap();
        assert_eq!(items.len(), 2);
        let names: Vec<_> = items.iter().map(|i| i.ingredient_name.as_str()).collect();
        assert_eq!(names, vec!["aubergine", "miso paste"]);
    }

    #[test]
    fn ghost_recipe_is_skipped() {
        let (_d, conn) = fresh();
        let rid = insert_recipe_with(&conn, "Gone", vec![line("x")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();
        crate::recipe::dal::soft_delete_recipe(&conn, &rid).unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.ghost_recipes_skipped, 1);
        assert_eq!(report.items_added, 0);
        assert!(dal::list_items(&conn).unwrap().is_empty());
    }

    #[test]
    fn no_meals_wipes_existing_generated_and_keeps_manual() {
        let (_d, conn) = fresh();
        // Pre-seed: a generated leftover + a manual row.
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        dal::insert_generated(&conn, "stale", None, None, "xxx", "xxx", 0).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let manual_id = dal::insert_manual(&conn, "bin bags").unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.items_added, 0);

        let list = dal::list_items(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, manual_id);
    }

    #[test]
    fn duplicate_ingredients_across_recipes_keep_both() {
        let (_d, conn) = fresh();
        let r1 = insert_recipe_with(&conn, "Miso", vec![line("onion")]);
        let r2 = insert_recipe_with(&conn, "Dal", vec![line("onion")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &r1).unwrap();
        crate::meal_plan::dal::set_entry(&conn, "2026-04-23", &r2).unwrap();

        let report = regenerate_from_week(&conn, "2026-04-20").unwrap();
        assert_eq!(report.items_added, 2);
        let items = dal::list_items(&conn).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].recipe_title.as_deref(), Some("Miso"));
        assert_eq!(items[1].recipe_title.as_deref(), Some("Dal"));
    }

    #[test]
    fn idempotent_when_called_twice() {
        let (_d, conn) = fresh();
        let rid = insert_recipe_with(&conn, "Miso", vec![line("onion"), line("aubergine")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();

        regenerate_from_week(&conn, "2026-04-20").unwrap();
        let first_count = dal::list_items(&conn).unwrap().len();
        regenerate_from_week(&conn, "2026-04-20").unwrap();
        let second_count = dal::list_items(&conn).unwrap().len();

        assert_eq!(first_count, 2);
        assert_eq!(second_count, 2);
    }

    #[test]
    fn manual_items_survive_and_stay_at_top_positions() {
        let (_d, conn) = fresh();
        let manual = dal::insert_manual(&conn, "bin bags").unwrap();
        let rid = insert_recipe_with(&conn, "Miso", vec![line("onion")]);
        crate::meal_plan::dal::set_entry(&conn, "2026-04-22", &rid).unwrap();

        regenerate_from_week(&conn, "2026-04-20").unwrap();
        let items = dal::list_items(&conn).unwrap();
        assert_eq!(items.len(), 2);
        // Manual row (position 0) stays first; generated item lands at position 1.
        assert_eq!(items[0].id, manual);
        assert_eq!(items[0].source, super::super::ItemSource::Manual);
        assert_eq!(items[1].ingredient_name, "onion");
        assert_eq!(items[1].source, super::super::ItemSource::Generated);
    }
}
