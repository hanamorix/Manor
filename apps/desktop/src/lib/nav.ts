import { create } from "zustand";

export type View = "today" | "chores" | "timeblocks";

interface NavStore {
  view: View;
  setView: (v: View) => void;
}

export const useNavStore = create<NavStore>((set) => ({
  view: "today",
  setView: (v) => set({ view: v }),
}));
