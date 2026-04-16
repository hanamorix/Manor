//! Tauri commands for Ledger — categories, transactions, budgets.

use crate::assistant::commands::Db;
use chrono::{Datelike, Local};
use manor_core::ledger::{budget, category, transaction};
use tauri::State;

fn current_year_month() -> (i32, u32) {
    let now = Local::now();
    (now.year(), now.month())
}

// ── Categories ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ledger_list_categories(state: State<'_, Db>) -> Result<Vec<category::Category>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    category::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpsertCategoryArgs {
    pub id: Option<i64>,
    pub name: String,
    pub emoji: String,
    #[serde(rename = "isIncome")]
    pub is_income: bool,
}

#[tauri::command]
pub fn ledger_upsert_category(
    state: State<'_, Db>,
    args: UpsertCategoryArgs,
) -> Result<category::Category, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(id) = args.id {
        category::update(&conn, id, &args.name, &args.emoji).map_err(|e| e.to_string())
    } else {
        category::insert(&conn, &args.name, &args.emoji, args.is_income)
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn ledger_delete_category(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    category::delete(&conn, id).map_err(|e| e.to_string())
}

// ── Transactions ──────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct AddTransactionArgs {
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
    pub currency: String,
    pub description: String,
    pub merchant: Option<String>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    pub date: i64,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_add_transaction(
    state: State<'_, Db>,
    args: AddTransactionArgs,
) -> Result<transaction::Transaction, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::insert(
        &conn,
        args.amount_pence,
        &args.currency,
        &args.description,
        args.merchant.as_deref(),
        args.category_id,
        args.date,
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateTransactionArgs {
    pub id: i64,
    pub description: String,
    pub merchant: Option<String>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_update_transaction(
    state: State<'_, Db>,
    args: UpdateTransactionArgs,
) -> Result<transaction::Transaction, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::update(
        &conn,
        args.id,
        &args.description,
        args.merchant.as_deref(),
        args.category_id,
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_transaction(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_list_transactions(
    state: State<'_, Db>,
    year: i32,
    month: u32,
) -> Result<Vec<transaction::Transaction>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    transaction::list_by_month(&conn, year, month).map_err(|e| e.to_string())
}

// ── Budgets ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ledger_list_budgets(state: State<'_, Db>) -> Result<Vec<budget::Budget>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpsertBudgetArgs {
    #[serde(rename = "categoryId")]
    pub category_id: i64,
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
}

#[tauri::command]
pub fn ledger_upsert_budget(
    state: State<'_, Db>,
    args: UpsertBudgetArgs,
) -> Result<budget::Budget, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::upsert(&conn, args.category_id, args.amount_pence).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_budget(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_monthly_summary(
    state: State<'_, Db>,
    year: i32,
    month: u32,
) -> Result<budget::MonthlySummary, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    budget::monthly_summary(&conn, year, month).map_err(|e| e.to_string())
}
