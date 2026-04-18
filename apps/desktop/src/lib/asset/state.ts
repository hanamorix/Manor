import { create } from "zustand";
import * as ipc from "./ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface AssetStore {
  assets: ipc.Asset[];
  search: string;
  category: ipc.AssetCategory | null;
  loadStatus: LoadStatus;

  load(): Promise<void>;
  setSearch(s: string): void;
  setCategory(c: ipc.AssetCategory | null): void;
}

export const useAssetStore = create<AssetStore>((set, get) => ({
  assets: [],
  search: "",
  category: null,
  loadStatus: { kind: "idle" },

  async load() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const assets = await ipc.list(get().search || undefined, get().category);
      set({ assets, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  setSearch(s) { set({ search: s }); void get().load(); },
  setCategory(c) { set({ category: c }); void get().load(); },
}));
