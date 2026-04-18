import { invoke } from "@tauri-apps/api/core";

export interface StapleItem {
  id: string;
  name: string;
  aliases: string[];
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface StapleDraft {
  name: string;
  aliases: string[];
}

export async function list(): Promise<StapleItem[]> {
  return await invoke<StapleItem[]>("staple_list");
}

export async function create(draft: StapleDraft): Promise<string> {
  return await invoke<string>("staple_create", { draft });
}

export async function update(id: string, draft: StapleDraft): Promise<void> {
  await invoke("staple_update", { id, draft });
}

export async function deleteStaple(id: string): Promise<void> {
  await invoke("staple_delete", { id });
}

export async function restore(id: string): Promise<void> {
  await invoke("staple_restore", { id });
}
