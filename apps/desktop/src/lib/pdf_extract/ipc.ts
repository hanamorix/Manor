import { invoke } from "@tauri-apps/api/core";
import type { Proposal } from "../today/ipc";
import type { MaintenanceScheduleDraft } from "../maintenance/ipc";

export interface ExtractOutcome {
  proposals_inserted: number;
  replaced_pending_count: number;
}

export async function extractOllama(attachmentUuid: string): Promise<ExtractOutcome> {
  return await invoke<ExtractOutcome>("pdf_extract_ollama", { attachmentUuid });
}

export async function extractClaude(attachmentUuid: string): Promise<ExtractOutcome> {
  return await invoke<ExtractOutcome>("pdf_extract_claude", { attachmentUuid });
}

export async function listPendingForAsset(assetId: string): Promise<Proposal[]> {
  return await invoke<Proposal[]>(
    "pdf_extract_pending_proposals_for_asset",
    { assetId },
  );
}

export async function pendingExistsForAttachment(attachmentUuid: string): Promise<boolean> {
  return await invoke<boolean>(
    "pdf_extract_pending_exists_for_attachment",
    { attachmentUuid },
  );
}

export async function approveAsIs(proposalId: number): Promise<string> {
  return await invoke<string>("pdf_extract_approve_as_is", { proposalId });
}

export async function reject(proposalId: number): Promise<void> {
  await invoke<void>("pdf_extract_reject", { proposalId });
}

export async function approveWithOverride(
  proposalId: number,
  draft: MaintenanceScheduleDraft,
): Promise<string> {
  return await invoke<string>(
    "pdf_extract_approve_with_override",
    { proposalId, draft },
  );
}
