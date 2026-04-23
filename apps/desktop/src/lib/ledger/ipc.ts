import { invoke } from "@tauri-apps/api/core";

export interface Category {
  id: number;
  name: string;
  emoji: string;
  is_income: boolean;
  sort_order: number;
  is_default: boolean;
  deleted_at: number | null;
}

export interface Transaction {
  id: number;
  bank_account_id: number | null;
  amount_pence: number;
  currency: string;
  description: string;
  merchant: string | null;
  category_id: number | null;
  date: number;
  source: "manual" | "sync";
  note: string | null;
  created_at: number;
}

export interface Budget {
  id: number;
  category_id: number;
  amount_pence: number;
  created_at: number;
}

export interface CategorySpend {
  category_id: number;
  category_name: string;
  category_emoji: string;
  spent_pence: number;
  budget_pence: number | null;
}

export interface MonthlySummary {
  total_in_pence: number;
  total_out_pence: number;
  by_category: CategorySpend[];
}

// Categories
export async function listCategories(): Promise<Category[]> {
  return invoke<Category[]>("ledger_list_categories");
}
export async function upsertCategory(args: {
  id?: number;
  name: string;
  emoji: string;
  isIncome: boolean;
}): Promise<Category> {
  return invoke<Category>("ledger_upsert_category", { args });
}
export async function deleteCategory(id: number): Promise<void> {
  return invoke<void>("ledger_delete_category", { id });
}

// Transactions
export async function listTransactions(year: number, month: number): Promise<Transaction[]> {
  return invoke<Transaction[]>("ledger_list_transactions", { year, month });
}
export async function addTransaction(args: {
  amountPence: number;
  currency: string;
  description: string;
  merchant?: string;
  categoryId?: number;
  date: number;
  note?: string;
}): Promise<Transaction> {
  return invoke<Transaction>("ledger_add_transaction", { args });
}
export async function updateTransaction(args: {
  id: number;
  description: string;
  merchant?: string;
  categoryId?: number;
  note?: string;
}): Promise<Transaction> {
  return invoke<Transaction>("ledger_update_transaction", { args });
}
export async function deleteTransaction(id: number): Promise<void> {
  return invoke<void>("ledger_delete_transaction", { id });
}

// Budgets
export async function listBudgets(): Promise<Budget[]> {
  return invoke<Budget[]>("ledger_list_budgets");
}
export async function upsertBudget(args: {
  categoryId: number;
  amountPence: number;
}): Promise<Budget> {
  return invoke<Budget>("ledger_upsert_budget", { args });
}
export async function deleteBudget(id: number): Promise<void> {
  return invoke<void>("ledger_delete_budget", { id });
}
export async function getMonthlySummary(
  year: number,
  month: number
): Promise<MonthlySummary> {
  return invoke<MonthlySummary>("ledger_monthly_summary", { year, month });
}

// Recurring payments
export interface RecurringPayment {
  id: number;
  description: string;
  amount_pence: number;
  currency: string;
  category_id: number | null;
  day_of_month: number;
  active: boolean;
  note: string | null;
  created_at: number;
}

export async function listRecurring(): Promise<RecurringPayment[]> {
  return invoke<RecurringPayment[]>("ledger_list_recurring");
}
export async function addRecurring(args: {
  description: string;
  amountPence: number;
  currency: string;
  categoryId?: number;
  dayOfMonth: number;
  note?: string;
}): Promise<RecurringPayment> {
  return invoke<RecurringPayment>("ledger_add_recurring", { args });
}
export async function updateRecurring(args: {
  id: number;
  description: string;
  amountPence: number;
  categoryId?: number;
  dayOfMonth: number;
  active: boolean;
  note?: string;
}): Promise<RecurringPayment> {
  return invoke<RecurringPayment>("ledger_update_recurring", { args });
}
export async function deleteRecurring(id: number): Promise<void> {
  return invoke<void>("ledger_delete_recurring", { id });
}

// Contracts
export interface Contract {
  id: number;
  provider: string;
  kind: "phone" | "broadband" | "insurance" | "energy" | "other";
  description: string | null;
  monthly_cost_pence: number;
  term_start: number;
  term_end: number;
  exit_fee_pence: number | null;
  renewal_alert_days: number;
  recurring_payment_id: number | null;
  note: string | null;
  created_at: number;
}

export interface RenewalAlert {
  contract_id: number;
  provider: string;
  kind: string;
  term_end: number;
  days_remaining: number;
  exit_fee_pence: number | null;
  severity: "amber" | "red";
}

export interface ContractArgs {
  provider: string;
  kind: string;
  description?: string;
  monthlyCostPence: number;
  termStart: number;
  termEnd: number;
  exitFeePence?: number;
  renewalAlertDays: number;
  recurringPaymentId?: number;
  note?: string;
}

export async function listContracts(): Promise<Contract[]> {
  return invoke<Contract[]>("ledger_list_contracts");
}
export async function addContract(args: ContractArgs): Promise<Contract> {
  return invoke<Contract>("ledger_add_contract", { args });
}
export async function updateContract(args: ContractArgs & { id: number }): Promise<Contract> {
  const { id, ...fields } = args;
  return invoke<Contract>("ledger_update_contract", { args: { id, fields } });
}
export async function deleteContract(id: number): Promise<void> {
  return invoke<void>("ledger_delete_contract", { id });
}
export async function getRenewalAlerts(): Promise<RenewalAlert[]> {
  return invoke<RenewalAlert[]>("ledger_get_renewal_alerts");
}

// CSV Import
export interface PreviewRow {
  date: number;
  amount_pence: number;
  description: string;
  suggested_category_id: number | null;
  duplicate: boolean;
}
export interface ImportResult {
  inserted: number;
  skipped_duplicates: number;
  skipped_errors: number;
}
export interface GenericCols { date: number; amount: number; description: number; }

export async function previewCsv(args: {
  preset: string;
  csvBytes: number[];
  genericCols?: GenericCols;
}): Promise<{ rows: PreviewRow[] }> {
  return invoke("ledger_preview_csv", { args });
}
export async function importCsv(rows: PreviewRow[]): Promise<ImportResult> {
  return invoke<ImportResult>("ledger_import_csv", { args: { rows } });
}

/// Suggest categories for recently-imported, uncategorised CSV rows via Ollama.
/// Best-effort; silently returns 0 if the local model is unreachable.
export async function autocatPending(): Promise<number> {
  return invoke<number>("ledger_bank_autocat_pending");
}

// AI Month Review — streams via Channel
import { Channel } from "@tauri-apps/api/core";
export type StreamChunk =
  | { type: "Token"; data: string }
  | { type: "Started"; data: number }
  | { type: "Done" }
  | { type: "Error"; data: string }
  | { type: "Proposal"; data: number };

export function aiMonthReview(
  args: { year: number; month: number },
  onEvent: (c: StreamChunk) => void
): Promise<void> {
  const ch = new Channel<StreamChunk>();
  ch.onmessage = onEvent;
  return invoke<void>("ledger_ai_month_review", { args, onEvent: ch });
}
