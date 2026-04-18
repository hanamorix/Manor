use crate::assistant::commands::Db;
use manor_core::meal_plan::{dal, staples, MealPlanEntry, StapleDraft, StapleItem};
use manor_core::recipe::{self, Recipe};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct MealPlanEntryWithRecipe {
    pub entry_date: String,
    pub recipe: Option<Recipe>,
}

fn load_entry_with_recipe(
    conn: &rusqlite::Connection,
    entry: MealPlanEntry,
) -> MealPlanEntryWithRecipe {
    let recipe = entry
        .recipe_id
        .as_ref()
        .and_then(|id| recipe::dal::get_recipe_including_trashed(conn, id).ok().flatten());
    MealPlanEntryWithRecipe {
        entry_date: entry.entry_date,
        recipe,
    }
}

#[tauri::command]
pub fn meal_plan_week_get(
    start_date: String,
    state: State<'_, Db>,
) -> Result<Vec<MealPlanEntryWithRecipe>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let entries = dal::get_week(&conn, &start_date).map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| load_entry_with_recipe(&conn, e))
        .collect())
}

#[tauri::command]
pub fn meal_plan_today_get(
    state: State<'_, Db>,
) -> Result<Option<MealPlanEntryWithRecipe>, String> {
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let entry = dal::get_entry(&conn, &today).map_err(|e| e.to_string())?;
    Ok(entry.map(|e| load_entry_with_recipe(&conn, e)))
}

#[tauri::command]
pub fn meal_plan_set_entry(
    date: String,
    recipe_id: String,
    state: State<'_, Db>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::set_entry(&conn, &date, &recipe_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn meal_plan_clear_entry(date: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::clear_entry(&conn, &date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_list(state: State<'_, Db>) -> Result<Vec<StapleItem>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::list_staples(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_create(draft: StapleDraft, state: State<'_, Db>) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::insert_staple(&conn, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_update(id: String, draft: StapleDraft, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::update_staple(&conn, &id, &draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::soft_delete_staple(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn staple_restore(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    staples::restore_staple(&conn, &id).map_err(|e| e.to_string())
}
