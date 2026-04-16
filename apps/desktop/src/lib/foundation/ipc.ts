import { invoke } from "@tauri-apps/api/core";

export async function settingGet(key: string): Promise<string | null> {
  return invoke<string | null>("setting_get", { key });
}

export async function settingSet(key: string, value: string): Promise<void> {
  return invoke<void>("setting_set", { key, value });
}

export async function settingListPrefixed(prefix: string): Promise<Array<[string, string]>> {
  return invoke<Array<[string, string]>>("setting_list_prefixed", { prefix });
}
