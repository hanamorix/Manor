import { create } from "zustand";
import * as ipc from "./staples-ipc";

interface StaplesStore {
  staples: ipc.StapleItem[];
  error: string | null;
  load(): Promise<void>;
  add(name: string): Promise<void>;
  updateOne(id: string, draft: ipc.StapleDraft): Promise<void>;
  remove(id: string): Promise<void>;
}

export const useStaplesStore = create<StaplesStore>((set, get) => ({
  staples: [],
  error: null,
  async load() {
    try { set({ staples: await ipc.list(), error: null }); }
    catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message });
    }
  },
  async add(name) {
    if (!name.trim()) return;
    await ipc.create({ name: name.trim(), aliases: [] });
    await get().load();
  },
  async updateOne(id, draft) { await ipc.update(id, draft); await get().load(); },
  async remove(id) { await ipc.deleteStaple(id); await get().load(); },
}));
