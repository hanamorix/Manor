import { invoke, convertFileSrc } from "@tauri-apps/api/core";

export type AssetCategory = "appliance" | "vehicle" | "fixture" | "other";

export interface Asset {
  id: string;
  name: string;
  category: AssetCategory;
  make: string | null;
  model: string | null;
  serial_number: string | null;
  purchase_date: string | null;
  notes: string;
  hero_attachment_uuid: string | null;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface AssetDraft {
  name: string;
  category: AssetCategory;
  make: string | null;
  model: string | null;
  serial_number: string | null;
  purchase_date: string | null;
  notes: string;
  hero_attachment_uuid: string | null;
}

export interface AttachmentSummary {
  id: number;
  uuid: string;
  original_name: string;
  mime_type: string;
  size_bytes: number;
  sha256: string;
  entity_type: string | null;
  entity_id: string | null;
  created_at: number;
}

export async function list(search?: string, category?: AssetCategory | null): Promise<Asset[]> {
  return await invoke<Asset[]>("asset_list", { args: { search, category } });
}

export async function get(id: string): Promise<Asset | null> {
  return await invoke<Asset | null>("asset_get", { id });
}

export async function create(draft: AssetDraft): Promise<string> {
  return await invoke<string>("asset_create", { draft });
}

export async function update(id: string, draft: AssetDraft): Promise<void> {
  await invoke("asset_update", { id, draft });
}

export async function deleteAsset(id: string): Promise<void> {
  await invoke("asset_delete", { id });
}

export async function restore(id: string): Promise<void> {
  await invoke("asset_restore", { id });
}

export async function attachHeroFromPath(id: string, sourcePath: string): Promise<string> {
  return await invoke<string>("asset_attach_hero_from_path", { id, sourcePath });
}

export async function attachDocumentFromPath(id: string, sourcePath: string): Promise<string> {
  return await invoke<string>("asset_attach_document_from_path", { id, sourcePath });
}

export async function listDocuments(id: string): Promise<AttachmentSummary[]> {
  return await invoke<AttachmentSummary[]>("asset_list_documents", { id });
}

/**
 * Resolve an attachment uuid to a webview-safe URL for rendering. Reuses the
 * attachment_get_path_by_uuid command shipped in L3a.
 */
export async function attachmentSrc(uuid: string): Promise<string> {
  const absPath = await invoke<string>("attachment_get_path_by_uuid", { uuid });
  return convertFileSrc(absPath);
}
