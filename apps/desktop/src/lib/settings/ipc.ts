import { invoke } from "@tauri-apps/api/core";

export interface CalendarAccount {
  id: number;
  display_name: string;
  server_url: string;
  username: string;
  last_synced_at: number | null;
  last_error: string | null;
  created_at: number;
  default_calendar_url: string | null;
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

export interface CalendarInfo {
  id: number;
  calendar_account_id: number;
  url: string;
  display_name: string | null;
}

export async function listCalendars(accountId: number): Promise<CalendarInfo[]> {
  return invoke("list_calendars", { accountId });
}

export async function setDefaultCalendar(accountId: number, url: string): Promise<void> {
  return invoke("set_default_calendar", { accountId, url });
}

export async function dataDirPath(): Promise<string> {
  return invoke<string>("data_dir_path");
}

export interface OllamaStatus {
  reachable: boolean;
  models: string[];
}

export async function ollamaStatus(): Promise<OllamaStatus> {
  return invoke<OllamaStatus>("ollama_status");
}

export interface EmbeddingsStatus {
  model: string;
  total: number;
  by_entity_type: Array<[string, number]>;
}

export async function embeddingsStatus(): Promise<EmbeddingsStatus> {
  return invoke<EmbeddingsStatus>("embeddings_status");
}

export async function embeddingsRebuild(): Promise<number> {
  return invoke<number>("embeddings_rebuild");
}

export interface SearchHit {
  entity_type: string;
  entity_id: number;
  score: number;
}

export async function embeddingsSearch(
  query: string,
  entityTypes: string[],
  limit: number,
): Promise<SearchHit[]> {
  return invoke<SearchHit[]>("embeddings_search", { query, entityTypes, limit });
}
