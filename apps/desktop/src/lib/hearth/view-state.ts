import { create } from "zustand";
import { settingGet, settingSet } from "../foundation/ipc";

export type HearthSubview = "recipes" | "this_week" | "staples";

interface HearthViewStore {
  subview: HearthSubview;
  hydrated: boolean;
  pendingDetailId: string | null;

  hydrate(): Promise<void>;
  setSubview(v: HearthSubview): void;
  openRecipeDetail(id: string): void;
  clearPendingDetail(): void;
}

export const useHearthViewStore = create<HearthViewStore>((set) => ({
  subview: "this_week",
  hydrated: false,
  pendingDetailId: null,

  async hydrate() {
    try {
      const v = await settingGet("hearth.last_subview");
      if (v === "recipes" || v === "this_week" || v === "staples") {
        set({ subview: v, hydrated: true });
      } else {
        set({ hydrated: true });
      }
    } catch { set({ hydrated: true }); }
  },
  setSubview(v) {
    set({ subview: v });
    void settingSet("hearth.last_subview", v).catch(() => {});
  },
  openRecipeDetail(id) {
    set({ subview: "recipes", pendingDetailId: id });
    void settingSet("hearth.last_subview", "recipes").catch(() => {});
  },
  clearPendingDetail() {
    set({ pendingDetailId: null });
  },
}));
