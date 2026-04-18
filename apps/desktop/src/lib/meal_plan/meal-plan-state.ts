import { create } from "zustand";
import * as ipc from "./meal-plan-ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface MealPlanStore {
  weekStart: string;
  entries: ipc.MealPlanEntryWithRecipe[];
  tonight: ipc.MealPlanEntryWithRecipe | null;
  loadStatus: LoadStatus;

  setWeekStart(date: string): void;
  loadWeek(): Promise<void>;
  loadTonight(): Promise<void>;
  setEntry(date: string, recipeId: string): Promise<void>;
  clearEntry(date: string): Promise<void>;
}

function mondayOf(date: Date): string {
  const d = new Date(date);
  const day = (d.getDay() + 6) % 7;
  d.setDate(d.getDate() - day);
  return d.toISOString().slice(0, 10);
}

export const useMealPlanStore = create<MealPlanStore>((set, get) => ({
  weekStart: mondayOf(new Date()),
  entries: [],
  tonight: null,
  loadStatus: { kind: "idle" },

  setWeekStart(date) { set({ weekStart: date }); void get().loadWeek(); },

  async loadWeek() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const entries = await ipc.weekGet(get().weekStart);
      set({ entries, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async loadTonight() {
    try { set({ tonight: await ipc.todayGet() }); } catch { /* swallow */ }
  },

  async setEntry(date, recipeId) {
    await ipc.setEntry(date, recipeId);
    await get().loadWeek();
    await get().loadTonight();
  },

  async clearEntry(date) {
    await ipc.clearEntry(date);
    await get().loadWeek();
    await get().loadTonight();
  },
}));
