import { create } from "zustand";
import type { TimeBlock, PatternSuggestion } from "./ipc";

interface TimeBlocksStore {
  todayBlocks: TimeBlock[];
  weekBlocks: TimeBlock[];
  recurringBlocks: TimeBlock[];
  patternSuggestion: PatternSuggestion | null;

  setTodayBlocks: (b: TimeBlock[]) => void;
  setWeekBlocks: (b: TimeBlock[]) => void;
  setRecurringBlocks: (b: TimeBlock[]) => void;
  setPatternSuggestion: (s: PatternSuggestion | null) => void;
  upsertBlock: (b: TimeBlock) => void;
  removeBlock: (id: number) => void;
}

export const useTimeBlocksStore = create<TimeBlocksStore>((set) => ({
  todayBlocks: [],
  weekBlocks: [],
  recurringBlocks: [],
  patternSuggestion: null,

  setTodayBlocks: (b) => set({ todayBlocks: b }),
  setWeekBlocks: (b) => set({ weekBlocks: b }),
  setRecurringBlocks: (b) => set({ recurringBlocks: b }),
  setPatternSuggestion: (s) => set({ patternSuggestion: s }),

  upsertBlock: (b) =>
    set((st) => {
      const updateList = (list: TimeBlock[]) => {
        const idx = list.findIndex((x) => x.id === b.id);
        if (idx === -1) return [...list, b];
        const next = list.slice();
        next[idx] = b;
        return next;
      };
      return {
        todayBlocks: updateList(st.todayBlocks),
        weekBlocks: updateList(st.weekBlocks),
      };
    }),

  removeBlock: (id) =>
    set((st) => ({
      todayBlocks: st.todayBlocks.filter((x) => x.id !== id),
      weekBlocks: st.weekBlocks.filter((x) => x.id !== id),
      recurringBlocks: st.recurringBlocks.filter((x) => x.id !== id),
    })),
}));
