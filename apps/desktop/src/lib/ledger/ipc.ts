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
