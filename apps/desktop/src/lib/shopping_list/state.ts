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
    // Optimistic: flip locally + re-sort to match backend ORDER BY ticked ASC, position ASC.
    // Revert on error — no reload needed when IPC succeeds.
    const before = get().items;
    const optimistic = before
      .map((i) => i.id === id ? { ...i, ticked: !i.ticked } : i)
      .sort((a, b) => {
        if (a.ticked !== b.ticked) return a.ticked ? 1 : -1;
        return a.position - b.position;
      });
    set({ items: optimistic });
    try {
      await ipc.toggle(id);
      // Local state already matches the backend — no reload needed.
    } catch (e: unknown) {
      // Revert on failure.
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
