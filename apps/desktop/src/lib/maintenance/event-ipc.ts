import { invoke } from "@tauri-apps/api/core";

export type EventSource = "manual" | "backfill";

export interface MaintenanceEvent {
  id: string;
  asset_id: string;
  schedule_id: string | null;
  title: string;
  completed_date: string;
  cost_pence: number | null;
  currency: string;
  notes: string;
  transaction_id: number | null;
  source: EventSource;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface EventWithContext {
  event: MaintenanceEvent;
  schedule_task: string | null;
  schedule_deleted: boolean;
  transaction_description: string | null;
  transaction_amount_pence: number | null;
  transaction_date: number | null;
}

export interface MaintenanceEventDraft {
  asset_id: string;
  schedule_id: string | null;
  title: string;
  completed_date: string;
  cost_pence: number | null;
  currency: string;
  notes: string;
  transaction_id: number | null;
}

export interface AssetSpendTotal {
  asset_id: string;
  asset_name: string;
  asset_category: string;
  total_last_12m_pence: number;
  total_lifetime_pence: number;
  event_count_last_12m: number;
  event_count_lifetime: number;
}

export interface CategorySpendTotal {
  category: string;
  total_last_12m_pence: number;
  total_lifetime_pence: number;
}

export interface LedgerTransaction {
  id: number;
  bank_account_id: number | null;
  amount_pence: number;
  currency: string;
  description: string;
  merchant: string | null;
  category_id: number | null;
  date: number;
  source: string;
  note: string | null;
  recurring_payment_id: number | null;
  created_at: number;
}

export async function listForAsset(assetId: string): Promise<EventWithContext[]> {
  return await invoke<EventWithContext[]>("maintenance_event_list_for_asset", { assetId });
}

export async function get(id: string): Promise<MaintenanceEvent | null> {
  return await invoke<MaintenanceEvent | null>("maintenance_event_get", { id });
}

export async function createOneOff(draft: MaintenanceEventDraft): Promise<string> {
  return await invoke<string>("maintenance_event_create_oneoff", { draft });
}

export async function logCompletion(scheduleId: string, draft: MaintenanceEventDraft): Promise<string> {
  return await invoke<string>("maintenance_event_log_completion", { scheduleId, draft });
}

export async function update(id: string, draft: MaintenanceEventDraft): Promise<void> {
  await invoke<void>("maintenance_event_update", { id, draft });
}

export async function assetSpendTotals(): Promise<AssetSpendTotal[]> {
  return await invoke<AssetSpendTotal[]>("maintenance_spend_asset_totals");
}

export async function spendForAsset(assetId: string): Promise<AssetSpendTotal> {
  return await invoke<AssetSpendTotal>("maintenance_spend_for_asset", { assetId });
}

export async function categorySpendTotals(): Promise<CategorySpendTotal[]> {
  return await invoke<CategorySpendTotal[]>("maintenance_spend_category_totals");
}

export async function suggestTransactions(
  completedDate: string,
  costPence: number | null,
  excludeEventId: string | null,
): Promise<LedgerTransaction[]> {
  return await invoke<LedgerTransaction[]>("maintenance_suggest_transactions", {
    completedDate,
    costPence,
    excludeEventId,
  });
}

export async function searchTransactions(query: string): Promise<LedgerTransaction[]> {
  return await invoke<LedgerTransaction[]>("maintenance_search_transactions", { query });
}
