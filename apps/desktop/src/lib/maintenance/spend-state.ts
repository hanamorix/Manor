import { create } from "zustand";
import * as eventIpc from "./event-ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface SpendStore {
  assetTotals: eventIpc.AssetSpendTotal[];
  categoryTotals: eventIpc.CategorySpendTotal[];
  loadStatus: LoadStatus;

  loadAssetTotals(): Promise<void>;
  loadCategoryTotals(): Promise<void>;
  refresh(): Promise<void>;
}

export const useSpendStore = create<SpendStore>((set, get) => ({
  assetTotals: [],
  categoryTotals: [],
  loadStatus: { kind: "idle" },

  async loadAssetTotals() {
    try {
      const rows = await eventIpc.assetSpendTotals();
      set({ assetTotals: rows });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
      console.error("spend-state: loadAssetTotals failed", message);
    }
  },

  async loadCategoryTotals() {
    try {
      const rows = await eventIpc.categorySpendTotals();
      set({ categoryTotals: rows });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
      console.error("spend-state: loadCategoryTotals failed", message);
    }
  },

  async refresh() {
    set({ loadStatus: { kind: "loading" } });
    await Promise.all([get().loadAssetTotals(), get().loadCategoryTotals()]);
    set({ loadStatus: { kind: "idle" } });
  },
}));
