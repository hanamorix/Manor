import { create } from "zustand";
import * as ipc from "./bank-ipc";

type SyncStatus =
  | { kind: "idle" }
  | { kind: "syncing"; account_id: number | null }
  | { kind: "error"; message: string };

interface BankStore {
  accounts: ipc.BankAccount[];
  syncStatus: SyncStatus;
  lastReport: ipc.SyncAccountReport[] | null;

  refresh(): Promise<void>;
  syncNow(account_id?: number): Promise<void>;
  disconnect(account_id: number): Promise<void>;
}

export const useBankStore = create<BankStore>((set) => ({
  accounts: [],
  syncStatus: { kind: "idle" },
  lastReport: null,

  async refresh() {
    const accounts = await ipc.listAccounts();
    set({ accounts });
  },

  async syncNow(account_id) {
    set({ syncStatus: { kind: "syncing", account_id: account_id ?? null } });
    try {
      const lastReport = await ipc.syncNow(account_id);
      const accounts = await ipc.listAccounts();
      set({ accounts, lastReport, syncStatus: { kind: "idle" } });
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      set({ syncStatus: { kind: "error", message } });
    }
  },

  async disconnect(account_id) {
    await ipc.disconnect(account_id);
    const accounts = await ipc.listAccounts();
    set({ accounts });
  },
}));
