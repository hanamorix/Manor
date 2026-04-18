use crate::assistant::commands::Db;
use manor_core::shopping_list::{dal, generator, GeneratedReport, ShoppingListItem};
use tauri::State;

#[tauri::command]
pub fn shopping_list_list(state: State<'_, Db>) -> Result<Vec<ShoppingListItem>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::list_items(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_add_manual(ingredient_name: String, state: State<'_, Db>) -> Result<String, String> {
    let name = ingredient_name.trim();
    if name.is_empty() {
        return Err("Ingredient name cannot be empty".into());
    }
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::insert_manual(&conn, name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_toggle(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::toggle_tick(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_delete(id: String, state: State<'_, Db>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    dal::delete_item(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shopping_list_regenerate(week_start: String, state: State<'_, Db>) -> Result<GeneratedReport, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    generator::regenerate_from_week(&conn, &week_start).map_err(|e| e.to_string())
}
