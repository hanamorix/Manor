import { create } from "zustand";
import * as ipc from "./ipc";
import type { Proposal } from "../today/ipc";
import type { MaintenanceScheduleDraft } from "../maintenance/ipc";

type ExtractStatus =
  | { kind: "idle" }
  | { kind: "extracting"; tier: "ollama" | "claude" }
  | { kind: "error"; message: string };

interface PdfExtractStore {
  proposalsByAsset: Record<string, Proposal[]>;
  pendingByAttachment: Record<string, boolean>;
  lastExtractMessage: string | null;
  extractStatus: ExtractStatus;

  loadForAsset(assetId: string): Promise<void>;
  loadPendingFlag(attachmentUuid: string): Promise<void>;
  extractOllama(attachmentUuid: string, assetId: string): Promise<void>;
  extractClaude(attachmentUuid: string, assetId: string): Promise<void>;
  approveAsIs(proposalId: number, assetId: string): Promise<void>;
  reject(proposalId: number, assetId: string): Promise<void>;
  approveWithOverride(
    proposalId: number,
    assetId: string,
    draft: MaintenanceScheduleDraft,
  ): Promise<void>;
  clearLastMessage(): void;
}

export const usePdfExtractStore = create<PdfExtractStore>((set, get) => ({
  proposalsByAsset: {},
  pendingByAttachment: {},
  lastExtractMessage: null,
  extractStatus: { kind: "idle" },

  async loadForAsset(assetId) {
    try {
      const rows = await ipc.listPendingForAsset(assetId);
      set((s) => ({ proposalsByAsset: { ...s.proposalsByAsset, [assetId]: rows } }));
    } catch (e: unknown) {
      console.error("pdf_extract: loadForAsset failed", e);
    }
  },

  async loadPendingFlag(attachmentUuid) {
    try {
      const exists = await ipc.pendingExistsForAttachment(attachmentUuid);
      set((s) => ({
        pendingByAttachment: { ...s.pendingByAttachment, [attachmentUuid]: exists },
      }));
    } catch (e: unknown) {
      console.error("pdf_extract: loadPendingFlag failed", e);
    }
  },

  async extractOllama(attachmentUuid, assetId) {
    set({ extractStatus: { kind: "extracting", tier: "ollama" } });
    try {
      const outcome = await ipc.extractOllama(attachmentUuid);
      await get().loadForAsset(assetId);
      await get().loadPendingFlag(attachmentUuid);
      set({
        extractStatus: { kind: "idle" },
        lastExtractMessage: describeOutcome(outcome),
      });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ extractStatus: { kind: "error", message } });
      throw e;
    }
  },

  async extractClaude(attachmentUuid, assetId) {
    set({ extractStatus: { kind: "extracting", tier: "claude" } });
    try {
      const outcome = await ipc.extractClaude(attachmentUuid);
      await get().loadForAsset(assetId);
      await get().loadPendingFlag(attachmentUuid);
      set({
        extractStatus: { kind: "idle" },
        lastExtractMessage: describeOutcome(outcome),
      });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ extractStatus: { kind: "error", message } });
      throw e;
    }
  },

  async approveAsIs(proposalId, assetId) {
    await ipc.approveAsIs(proposalId);
    await get().loadForAsset(assetId);
    set({ pendingByAttachment: {} });
  },

  async reject(proposalId, assetId) {
    await ipc.reject(proposalId);
    await get().loadForAsset(assetId);
    set({ pendingByAttachment: {} });
  },

  async approveWithOverride(proposalId, assetId, draft) {
    await ipc.approveWithOverride(proposalId, draft);
    await get().loadForAsset(assetId);
    set({ pendingByAttachment: {} });
  },

  clearLastMessage() {
    set({ lastExtractMessage: null });
  },
}));

function describeOutcome(outcome: ipc.ExtractOutcome): string {
  if (outcome.proposals_inserted === 0) {
    return "No maintenance schedules found in this manual.";
  }
  const base = `${outcome.proposals_inserted} proposal${outcome.proposals_inserted === 1 ? "" : "s"} extracted`;
  if (outcome.replaced_pending_count > 0) {
    return `${base}. ${outcome.replaced_pending_count} previous proposal${outcome.replaced_pending_count === 1 ? "" : "s"} replaced.`;
  }
  return `${base}.`;
}
