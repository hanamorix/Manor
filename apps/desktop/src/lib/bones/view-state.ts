import { create } from "zustand";
import { settingGet, settingSet } from "../foundation/ipc";

export type BonesSubview = "assets" | "due_soon";

interface BonesViewStore {
  subview: BonesSubview;
  hydrated: boolean;
  pendingAssetDetailId: string | null;

  hydrate(): Promise<void>;
  setSubview(v: BonesSubview): void;
  openAssetDetail(id: string): void;
  clearPendingDetail(): void;
}

export const useBonesViewStore = create<BonesViewStore>((set) => ({
  subview: "assets",
  hydrated: false,
  pendingAssetDetailId: null,

  async hydrate() {
    try {
      const v = await settingGet("bones.last_subview");
      if (v === "assets" || v === "due_soon") {
        set({ subview: v, hydrated: true });
      } else {
        set({ hydrated: true });
      }
    } catch {
      set({ hydrated: true });
    }
  },
  setSubview(v) {
    set({ subview: v });
    void settingSet("bones.last_subview", v).catch(() => {});
  },
  openAssetDetail(id) {
    set({ subview: "assets", pendingAssetDetailId: id });
    void settingSet("bones.last_subview", "assets").catch(() => {});
  },
  clearPendingDetail() {
    set({ pendingAssetDetailId: null });
  },
}));
