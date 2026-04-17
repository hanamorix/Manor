import { invoke } from "@tauri-apps/api/core";

export interface RemoteProviderStatus {
  provider: string;
  has_key: boolean;
  budget_pence: number;
  spent_month_pence: number;
  enabled_for_review: boolean;
}

export interface CallLogEntry {
  id: number;
  provider: string;
  model: string;
  skill: string;
  user_visible_reason: string;
  prompt_redacted: string;
  response_text: string | null;
  input_tokens: number | null;
  output_tokens: number | null;
  cost_pence: number | null;
  redaction_count: number;
  error: string | null;
  started_at: number;
  completed_at: number | null;
}

export async function remoteProviderStatus(): Promise<RemoteProviderStatus> {
  return invoke<RemoteProviderStatus>("remote_provider_status");
}
export async function remoteSetKey(provider: string, key: string): Promise<void> {
  return invoke<void>("remote_set_key", { provider, key });
}
export async function remoteRemoveKey(provider: string): Promise<boolean> {
  return invoke<boolean>("remote_remove_key", { provider });
}
export async function remoteSetBudget(provider: string, pence: number): Promise<void> {
  return invoke<void>("remote_set_budget", { provider, pence });
}
export async function remoteSetEnabledForReview(enabled: boolean): Promise<void> {
  return invoke<void>("remote_set_enabled_for_review", { enabled });
}
export async function remoteCallLogList(limit: number): Promise<CallLogEntry[]> {
  return invoke<CallLogEntry[]>("remote_call_log_list", { limit });
}
export async function remoteCallLogClear(): Promise<number> {
  return invoke<number>("remote_call_log_clear");
}
export async function remoteTest(): Promise<string> {
  return invoke<string>("remote_test");
}
