import { create } from "zustand";
import * as ipc from "./ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface ShoppingListStore {
  items: ipc.ShoppingListItem[];
  loadStatus: LoadStatus;
  lastReport: ipc.GeneratedReport | null;

  load(): Promise<void>;
  toggle(id: string): Promise<void>;
  addManual(name: string): Promise<void>;
  deleteItem(id: string): Promise<void>;
  regenerate(weekStart: string): Promise<ipc.GeneratedReport>;
}

export const useShoppingListStore = create<ShoppingListStore>((set, get) => ({
  items: [],
  loadStatus: { kind: "idle" },
  lastReport: null,

  async load() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const items = await ipc.list();
      set({ items, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async toggle(id) {
    // Optimistic: flip locally, revert on error.
    const before = get().items;
    set({
      items: before.map((i) => i.id === id ? { ...i, ticked: !i.ticked } : i),
    });
    try {
      await ipc.toggle(id);
      await get().load();
    } catch (e: unknown) {
      set({ items: before });
      throw e;
    }
  },

  async addManual(name) {
    await ipc.addManual(name);
    await get().load();
  },

  async deleteItem(id) {
    await ipc.deleteItem(id);
    await get().load();
  },

  async regenerate(weekStart) {
    const report = await ipc.regenerate(weekStart);
    set({ lastReport: report });
    await get().load();
    return report;
  },
}));
