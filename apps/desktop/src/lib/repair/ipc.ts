import { invoke } from "@tauri-apps/api/core";

export type LlmTier = "ollama" | "claude";

export interface RepairSource {
  url: string;
  title: string;
}

export interface RepairNote {
  id: string;
  asset_id: string;
  symptom: string;
  body_md: string;
  sources: RepairSource[];
  video_sources: RepairSource[] | null;
  tier: LlmTier;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface PipelineOutcome {
  note: RepairNote | null;
  sources: RepairSource[];
  video_sources: RepairSource[];
  empty_or_failed: boolean;
}

export async function searchOllama(assetId: string, symptom: string): Promise<PipelineOutcome> {
  return await invoke<PipelineOutcome>("repair_search_ollama", { assetId, symptom });
}

export async function searchClaude(assetId: string, symptom: string): Promise<PipelineOutcome> {
  return await invoke<PipelineOutcome>("repair_search_claude", { assetId, symptom });
}

export async function listForAsset(assetId: string): Promise<RepairNote[]> {
  return await invoke<RepairNote[]>("repair_note_list_for_asset", { assetId });
}

export async function get(id: string): Promise<RepairNote | null> {
  return await invoke<RepairNote | null>("repair_note_get", { id });
}

export async function deleteNote(id: string): Promise<void> {
  await invoke<void>("repair_note_delete", { id });
}
