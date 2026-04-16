import { create } from "zustand";
import type { Chore, FairnessNudge } from "./ipc";

interface ChoresStore {
  choresDueToday: Chore[];
  allChores: Chore[];
  fairnessNudges: FairnessNudge[];

  setChoresDueToday: (c: Chore[]) => void;
  setAllChores: (c: Chore[]) => void;
  setFairnessNudges: (n: FairnessNudge[]) => void;
  upsertChore: (c: Chore) => void;
  removeChore: (id: number) => void;
  removeFromDueToday: (id: number) => void;
  dismissFairnessNudge: (choreId: number) => void;
}

export const useChoresStore = create<ChoresStore>((set) => ({
  choresDueToday: [],
  allChores: [],
  fairnessNudges: [],

  setChoresDueToday: (c) => set({ choresDueToday: c }),
  setAllChores: (c) => set({ allChores: c }),
  setFairnessNudges: (n) => set({ fairnessNudges: n }),

  upsertChore: (c) =>
    set((st) => {
      const updateList = (list: Chore[]) => {
        const idx = list.findIndex((x) => x.id === c.id);
        if (idx === -1) return [...list, c];
        const next = list.slice();
        next[idx] = c;
        return next;
      };
      return {
        allChores: updateList(st.allChores),
        choresDueToday: updateList(st.choresDueToday),
      };
    }),

  removeChore: (id) =>
    set((st) => ({
      allChores: st.allChores.filter((x) => x.id !== id),
      choresDueToday: st.choresDueToday.filter((x) => x.id !== id),
    })),

  removeFromDueToday: (id) =>
    set((st) => ({ choresDueToday: st.choresDueToday.filter((x) => x.id !== id) })),

  dismissFairnessNudge: (choreId) =>
    set((st) => ({ fairnessNudges: st.fairnessNudges.filter((n) => n.chore_id !== choreId) })),
}));
