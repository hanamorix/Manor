import { create } from "zustand";
import * as ipc from "./ipc";

type LoadStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string };

interface MaintenanceStore {
  dueSoon: ipc.ScheduleWithAsset[];
  dueTodayAndOverdue: ipc.ScheduleWithAsset[];
  schedulesByAsset: Record<string, ipc.MaintenanceSchedule[]>;
  overdueCount: number;
  loadStatus: LoadStatus;

  loadDueSoon(): Promise<void>;
  loadDueTodayAndOverdue(): Promise<void>;
  loadForAsset(assetId: string): Promise<void>;
  loadOverdueCount(): Promise<void>;

  create(draft: ipc.MaintenanceScheduleDraft): Promise<string>;
  update(id: string, draft: ipc.MaintenanceScheduleDraft): Promise<void>;
  markDone(id: string): Promise<void>;
  deleteSchedule(id: string): Promise<void>;
}

export const useMaintenanceStore = create<MaintenanceStore>((set, get) => ({
  dueSoon: [],
  dueTodayAndOverdue: [],
  schedulesByAsset: {},
  overdueCount: 0,
  loadStatus: { kind: "idle" },

  async loadDueTodayAndOverdue() {
    try {
      const rows = await ipc.dueTodayAndOverdue();
      set({ dueTodayAndOverdue: rows });
    } catch { /* swallow */ }
  },

  async loadDueSoon() {
    set({ loadStatus: { kind: "loading" } });
    try {
      const dueSoon = await ipc.dueSoon();
      set({ dueSoon, loadStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async loadForAsset(assetId) {
    try {
      const rows = await ipc.listForAsset(assetId);
      set((s) => ({ schedulesByAsset: { ...s.schedulesByAsset, [assetId]: rows } }));
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ loadStatus: { kind: "error", message } });
    }
  },

  async loadOverdueCount() {
    try { set({ overdueCount: await ipc.overdueCount() }); } catch { /* swallow */ }
  },

  async create(draft) {
    const id = await ipc.create(draft);
    await get().loadDueSoon();
    await get().loadDueTodayAndOverdue();
    await get().loadForAsset(draft.asset_id);
    await get().loadOverdueCount();
    return id;
  },

  async update(id, draft) {
    await ipc.update(id, draft);
    await get().loadDueSoon();
    await get().loadDueTodayAndOverdue();
    await get().loadForAsset(draft.asset_id);
    await get().loadOverdueCount();
  },

  async markDone(id) {
    const sch = (await ipc.get(id));
    await ipc.markDone(id);
    await get().loadDueSoon();
    await get().loadDueTodayAndOverdue();
    if (sch) await get().loadForAsset(sch.asset_id);
    await get().loadOverdueCount();
  },

  async deleteSchedule(id) {
    const sch = (await ipc.get(id));
    await ipc.deleteSchedule(id);
    await get().loadDueSoon();
    await get().loadDueTodayAndOverdue();
    if (sch) await get().loadForAsset(sch.asset_id);
    await get().loadOverdueCount();
  },
}));
