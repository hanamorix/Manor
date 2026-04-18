import { create } from "zustand";
import * as ipc from "./recipe-ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface RecipeStore {
  recipes: ipc.Recipe[];
  search: string;
  tagIds: string[];
  loadStatus: LoadStatus;

  load(): Promise<void>;
  setSearch(s: string): void;
  setTagIds(ids: string[]): void;
}

export const useRecipeStore = create<RecipeStore>((set, get) => ({
  recipes: [],
  search: "",
  tagIds: [],
  loadStatus: { kind: "idle" },

  async load() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const recipes = await ipc.list(get().search || undefined, get().tagIds);
      set({ recipes, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  setSearch(s: string) {
    set({ search: s });
    void get().load();
  },

  setTagIds(ids: string[]) {
    set({ tagIds: ids });
    void get().load();
  },
}));
