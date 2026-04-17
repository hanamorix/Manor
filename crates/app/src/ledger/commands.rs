//! Tauri commands for Ledger — categories, transactions, budgets.

use crate::assistant::commands::Db;
use manor_core::ledger::{budget, category, contract, recurring, transaction};
use tauri::State;

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
        category::insert(&conn, &args.name, &args.emoji, args.is_income).map_err(|e| e.to_string())
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

// ── Recurring payments ────────────────────────────────────────────────────────

#[tauri::command]
pub fn ledger_list_recurring(
    state: State<'_, Db>,
) -> Result<Vec<recurring::RecurringPayment>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct AddRecurringArgs {
    pub description: String,
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
    pub currency: String,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    #[serde(rename = "dayOfMonth")]
    pub day_of_month: i64,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_add_recurring(
    state: State<'_, Db>,
    args: AddRecurringArgs,
) -> Result<recurring::RecurringPayment, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::insert(
        &conn,
        &args.description,
        args.amount_pence,
        &args.currency,
        args.category_id,
        args.day_of_month,
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateRecurringArgs {
    pub id: i64,
    pub description: String,
    #[serde(rename = "amountPence")]
    pub amount_pence: i64,
    #[serde(rename = "categoryId")]
    pub category_id: Option<i64>,
    #[serde(rename = "dayOfMonth")]
    pub day_of_month: i64,
    pub active: bool,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ledger_update_recurring(
    state: State<'_, Db>,
    args: UpdateRecurringArgs,
) -> Result<recurring::RecurringPayment, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::update(
        &conn,
        args.id,
        &args.description,
        args.amount_pence,
        args.category_id,
        args.day_of_month,
        args.active,
        args.note.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_recurring(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    recurring::delete(&conn, id).map_err(|e| e.to_string())
}

// ── Contracts ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ledger_list_contracts(state: State<'_, Db>) -> Result<Vec<contract::Contract>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::list(&conn).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct ContractArgs {
    pub provider: String,
    pub kind: String,
    pub description: Option<String>,
    #[serde(rename = "monthlyCostPence")]
    pub monthly_cost_pence: i64,
    #[serde(rename = "termStart")]
    pub term_start: i64,
    #[serde(rename = "termEnd")]
    pub term_end: i64,
    #[serde(rename = "exitFeePence")]
    pub exit_fee_pence: Option<i64>,
    #[serde(rename = "renewalAlertDays")]
    pub renewal_alert_days: i64,
    #[serde(rename = "recurringPaymentId")]
    pub recurring_payment_id: Option<i64>,
    pub note: Option<String>,
}

impl<'a> ContractArgs {
    fn as_new(&'a self) -> contract::NewContract<'a> {
        contract::NewContract {
            provider: &self.provider,
            kind: &self.kind,
            description: self.description.as_deref(),
            monthly_cost_pence: self.monthly_cost_pence,
            term_start: self.term_start,
            term_end: self.term_end,
            exit_fee_pence: self.exit_fee_pence,
            renewal_alert_days: self.renewal_alert_days,
            recurring_payment_id: self.recurring_payment_id,
            note: self.note.as_deref(),
        }
    }
}

#[tauri::command]
pub fn ledger_add_contract(
    state: State<'_, Db>,
    args: ContractArgs,
) -> Result<contract::Contract, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::insert(&conn, args.as_new()).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct UpdateContractArgs {
    pub id: i64,
    #[serde(flatten)]
    pub fields: ContractArgs,
}

#[tauri::command]
pub fn ledger_update_contract(
    state: State<'_, Db>,
    args: UpdateContractArgs,
) -> Result<contract::Contract, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::update(&conn, args.id, args.fields.as_new()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_delete_contract(state: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    contract::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ledger_get_renewal_alerts(
    state: State<'_, Db>,
) -> Result<Vec<contract::RenewalAlert>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    contract::check_renewals(&conn, now).map_err(|e| e.to_string())
}

// ── CSV Import ────────────────────────────────────────────────────────────────

use crate::ledger::csv_import::{self, BankPreset, GenericCols, ImportResult, PreviewRow};

#[derive(serde::Deserialize)]
pub struct ImportCsvArgs {
    pub preset: String,
    #[serde(rename = "csvBytes")]
    pub csv_bytes: Vec<u8>,
    #[serde(rename = "genericCols")]
    pub generic_cols: Option<GenericCols>,
}

#[derive(serde::Serialize)]
pub struct PreviewResponse {
    pub rows: Vec<PreviewRow>,
}

#[tauri::command]
pub fn ledger_preview_csv(
    state: State<'_, Db>,
    args: ImportCsvArgs,
) -> Result<PreviewResponse, String> {
    let preset = BankPreset::from_str(&args.preset).ok_or("unknown preset")?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let rows = csv_import::parse_preview(&conn, preset, &args.csv_bytes, args.generic_cols)
        .map_err(|e| e.to_string())?;
    Ok(PreviewResponse { rows })
}

#[derive(serde::Deserialize)]
pub struct DoImportArgs {
    pub rows: Vec<PreviewRow>,
}

#[tauri::command]
pub fn ledger_import_csv(state: State<'_, Db>, args: DoImportArgs) -> Result<ImportResult, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    csv_import::do_import(&mut conn, args.rows).map_err(|e| e.to_string())
}
