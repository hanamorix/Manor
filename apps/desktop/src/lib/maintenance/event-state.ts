import { create } from "zustand";
import * as eventIpc from "./event-ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface MaintenanceEventsStore {
  eventsByAsset: Record<string, eventIpc.EventWithContext[]>;
  loadStatus: LoadStatus;

  loadForAsset(assetId: string): Promise<void>;
  invalidateAsset(assetId: string): void;
  createOneOff(draft: eventIpc.MaintenanceEventDraft): Promise<string>;
  logCompletion(scheduleId: string, draft: eventIpc.MaintenanceEventDraft): Promise<string>;
  update(id: string, draft: eventIpc.MaintenanceEventDraft): Promise<void>;
  suggestTransactions(
    completedDate: string,
    costPence: number | null,
    excludeEventId: string | null,
  ): Promise<eventIpc.LedgerTransaction[]>;
  searchTransactions(query: string): Promise<eventIpc.LedgerTransaction[]>;
}

export const useMaintenanceEventsStore = create<MaintenanceEventsStore>((set, get) => ({
  eventsByAsset: {},
  loadStatus: { kind: "idle" },

  async loadForAsset(assetId) {
    set({ loadStatus: { kind: "loading" } });
    try {
      const rows = await eventIpc.listForAsset(assetId);
      set((s) => ({
        eventsByAsset: { ...s.eventsByAsset, [assetId]: rows },
        loadStatus: { kind: "idle" },
      }));
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
      console.error("event-state: loadForAsset failed", message);
    }
  },

  invalidateAsset(assetId) {
    set((s) => {
      const next = { ...s.eventsByAsset };
      delete next[assetId];
      return { eventsByAsset: next };
    });
  },

  async createOneOff(draft) {
    const id = await eventIpc.createOneOff(draft);
    get().invalidateAsset(draft.asset_id);
    return id;
  },

  async logCompletion(scheduleId, draft) {
    const id = await eventIpc.logCompletion(scheduleId, draft);
    get().invalidateAsset(draft.asset_id);
    return id;
  },

  async update(id, draft) {
    await eventIpc.update(id, draft);
    get().invalidateAsset(draft.asset_id);
  },

  suggestTransactions: (completedDate, costPence, excludeEventId) =>
    eventIpc.suggestTransactions(completedDate, costPence, excludeEventId),

  searchTransactions: (query) => eventIpc.searchTransactions(query),
}));
