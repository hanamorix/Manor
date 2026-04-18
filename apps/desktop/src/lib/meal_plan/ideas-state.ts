import { create } from "zustand";
import * as ipc from "./ideas-ipc";
import type { Recipe } from "../recipe/recipe-ipc";

export type IdeasMode = "library" | "llm";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface IdeasStore {
  mode: IdeasMode;
  library: Recipe[];
  llm: ipc.IdeaTitle[];
  loadStatus: LoadStatus;

  loadLibrary(): Promise<void>;
  loadLlm(): Promise<void>;
  backToLibrary(): void;
  expandAiTitle(t: ipc.IdeaTitle): Promise<ipc.ImportPreview>;
}

export const useIdeasStore = create<IdeasStore>((set, _get) => ({
  mode: "library",
  library: [],
  llm: [],
  loadStatus: { kind: "idle" },

  async loadLibrary() {
    set({ mode: "library", loadStatus: { kind: "loading" } });
    try {
      const library = await ipc.librarySample();
      set({ library, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async loadLlm() {
    set({ mode: "llm", loadStatus: { kind: "loading" }, llm: [] });
    try {
      const llm = await ipc.llmTitles();
      set({ llm, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      // Auto-switch back to library on LLM failure.
      set({ mode: "library", loadStatus: { kind: "error", message } });
    }
  },

  backToLibrary() {
    set({ mode: "library", loadStatus: { kind: "idle" } });
  },

  async expandAiTitle(t) {
    return await ipc.llmExpand(t.title, t.blurb);
  },
}));
