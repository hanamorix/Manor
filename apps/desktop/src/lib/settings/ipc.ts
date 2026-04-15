import { invoke } from "@tauri-apps/api/core";

export interface CalendarAccount {
  id: number;
  display_name: string;
  server_url: string;
  username: string;
  last_synced_at: number | null;
  last_error: string | null;
  created_at: number;
}

export interface SyncResult {
  account_id: number;
  events_added: number;
  error: string | null;
  synced_at: number;
}

export async function listCalendarAccounts(): Promise<CalendarAccount[]> {
  return invoke<CalendarAccount[]>("list_calendar_accounts");
}

export async function addCalendarAccount(
  serverUrl: string,
  username: string,
  password: string,
): Promise<CalendarAccount> {
  return invoke<CalendarAccount>("add_calendar_account", { serverUrl, username, password });
}

export async function removeCalendarAccount(id: number): Promise<void> {
  return invoke<void>("remove_calendar_account", { id });
}

export async function syncAccount(id: number): Promise<SyncResult> {
  return invoke<SyncResult>("sync_account", { id });
}

export async function syncAllAccounts(): Promise<SyncResult[]> {
  return invoke<SyncResult[]>("sync_all_accounts");
}
