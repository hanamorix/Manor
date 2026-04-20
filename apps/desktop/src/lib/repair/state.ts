import { create } from "zustand";
import * as ipc from "./ipc";

type SearchStatus =
  | { kind: "idle" }
  | { kind: "searching"; tier: ipc.LlmTier }
  | { kind: "error"; message: string };

interface RepairStore {
  notesByAsset: Record<string, ipc.RepairNote[]>;
  lastOutcomeByAsset: Record<string, ipc.PipelineOutcome | null>;
  lastSymptomByAsset: Record<string, string>;
  searchStatus: SearchStatus;

  loadForAsset(assetId: string): Promise<void>;
  invalidateAsset(assetId: string): void;
  searchOllama(assetId: string, symptom: string): Promise<ipc.PipelineOutcome>;
  searchClaude(assetId: string, symptom: string): Promise<ipc.PipelineOutcome>;
  deleteNote(id: string, assetId: string): Promise<void>;
  clearLastOutcome(assetId: string): void;
}

export const useRepairStore = create<RepairStore>((set, get) => ({
  notesByAsset: {},
  lastOutcomeByAsset: {},
  lastSymptomByAsset: {},
  searchStatus: { kind: "idle" },

  async loadForAsset(assetId) {
    try {
      const rows = await ipc.listForAsset(assetId);
      set((s) => ({ notesByAsset: { ...s.notesByAsset, [assetId]: rows } }));
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      console.error("repair-state: loadForAsset failed", message);
    }
  },

  invalidateAsset(assetId) {
    set((s) => {
      const next = { ...s.notesByAsset };
      delete next[assetId];
      return { notesByAsset: next };
    });
  },

  async searchOllama(assetId, symptom) {
    set((s) => ({
      searchStatus: { kind: "searching", tier: "ollama" },
      lastSymptomByAsset: { ...s.lastSymptomByAsset, [assetId]: symptom },
    }));
    try {
      const outcome = await ipc.searchOllama(assetId, symptom);
      set((s) => ({
        lastOutcomeByAsset: { ...s.lastOutcomeByAsset, [assetId]: outcome },
        searchStatus: { kind: "idle" },
      }));
      if (outcome.note) get().invalidateAsset(assetId);
      return outcome;
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ searchStatus: { kind: "error", message } });
      throw e;
    }
  },

  async searchClaude(assetId, symptom) {
    set((s) => ({
      searchStatus: { kind: "searching", tier: "claude" },
      lastSymptomByAsset: { ...s.lastSymptomByAsset, [assetId]: symptom },
    }));
    try {
      const outcome = await ipc.searchClaude(assetId, symptom);
      set((s) => ({
        lastOutcomeByAsset: { ...s.lastOutcomeByAsset, [assetId]: outcome },
        searchStatus: { kind: "idle" },
      }));
      if (outcome.note) get().invalidateAsset(assetId);
      return outcome;
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ searchStatus: { kind: "error", message } });
      throw e;
    }
  },

  async deleteNote(id, assetId) {
    await ipc.deleteNote(id);
    get().invalidateAsset(assetId);
  },

  clearLastOutcome(assetId) {
    set((s) => {
      const nextOutcome = { ...s.lastOutcomeByAsset };
      delete nextOutcome[assetId];
      return { lastOutcomeByAsset: nextOutcome };
    });
  },
}));
