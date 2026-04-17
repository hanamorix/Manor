import { invoke } from "@tauri-apps/api/core";
import { open as shellOpen } from "@tauri-apps/plugin-shell";

export interface BankAccount {
  id: number;
  provider: string;
  institution_name: string;
  institution_id: string | null;
  institution_logo_url: string | null;
  account_name: string;
  account_type: string;
  currency: string;
  external_id: string;
  requisition_id: string | null;
  requisition_expires_at: number | null;
  last_synced_at: number | null;
  sync_paused_reason: string | null;
  initial_sync_completed_at: number | null;
  created_at: number;
}

export interface UiInstitution {
  id: string;
  name: string;
  logo_url: string | null;
  is_sandbox: boolean;
}

export interface BeginConnectResponse {
  auth_url: string;
  requisition_id: string;
  reference: string;
  port: number;
  max_historical_days_granted: number;
  institution_id: string;
}

export interface SyncAccountReport {
  account_id: number;
  inserted: number;
  categorized: number;
  merged: number;
  skipped: boolean;
  error: string | null;
}

export interface BankCmdError {
  code: string;
  message: string;
}

export async function credentialsStatus(): Promise<boolean> {
  return await invoke<boolean>("ledger_bank_credentials_status");
}

export async function saveCredentials(
  secret_id: string,
  secret_key: string,
): Promise<void> {
  await invoke("ledger_bank_save_credentials", { args: { secret_id, secret_key } });
}

export async function listInstitutions(country: string): Promise<UiInstitution[]> {
  return await invoke<UiInstitution[]>("ledger_bank_list_institutions", { country });
}

export async function beginConnect(institution_id: string): Promise<BeginConnectResponse> {
  return await invoke<BeginConnectResponse>("ledger_bank_begin_connect", {
    args: { institution_id },
  });
}

export async function completeConnect(args: {
  reference: string;
  requisition_id: string;
  institution_id: string;
  institution_name: string;
  institution_logo_url: string | null;
  max_historical_days_granted: number;
  replaces_account_id?: number | null;
}): Promise<{ account_ids: number[] }> {
  return await invoke("ledger_bank_complete_connect", { args });
}

export async function listAccounts(): Promise<BankAccount[]> {
  return await invoke<BankAccount[]>("ledger_bank_list_accounts");
}

export async function syncNow(account_id?: number): Promise<SyncAccountReport[]> {
  return await invoke<SyncAccountReport[]>("ledger_bank_sync_now", {
    args: { account_id: account_id ?? null },
  });
}

export async function disconnect(account_id: number): Promise<void> {
  await invoke("ledger_bank_disconnect", { args: { account_id } });
}

export async function reconnect(account_id: number): Promise<BeginConnectResponse> {
  return await invoke<BeginConnectResponse>("ledger_bank_reconnect", {
    args: { account_id },
  });
}

export async function autocatPending(): Promise<number> {
  return await invoke<number>("ledger_bank_autocat_pending");
}

export async function openAuthUrl(url: string): Promise<void> {
  await shellOpen(url);
}
