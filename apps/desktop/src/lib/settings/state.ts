import { create } from "zustand";
import type { CalendarAccount, CalendarInfo } from "./ipc";

interface SettingsStore {
  modalOpen: boolean;
  activeTab: "data" | "ai" | "calendars" | "household" | "hearth" | "about";
  accounts: CalendarAccount[];
  syncingAccountIds: Set<number>;
  accountCalendars: Map<number, CalendarInfo[]>;

  setModalOpen: (open: boolean) => void;
  setActiveTab: (t: SettingsStore["activeTab"]) => void;
  setAccounts: (a: CalendarAccount[]) => void;
  upsertAccount: (a: CalendarAccount) => void;
  removeAccount: (id: number) => void;
  markSyncing: (id: number) => void;
  markSynced: (id: number) => void;
  setCalendars: (accountId: number, calendars: CalendarInfo[]) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  modalOpen: false,
  activeTab: "data",
  accounts: [],
  syncingAccountIds: new Set<number>(),
  accountCalendars: new Map<number, CalendarInfo[]>(),

  setModalOpen: (open) => set({ modalOpen: open }),
  setActiveTab: (t) => set({ activeTab: t }),
  setAccounts: (a) => set({ accounts: a }),

  upsertAccount: (a) =>
    set((st) => {
      const idx = st.accounts.findIndex((x) => x.id === a.id);
      if (idx === -1) return { accounts: [...st.accounts, a] };
      const next = st.accounts.slice();
      next[idx] = a;
      return { accounts: next };
    }),

  removeAccount: (id) => set((st) => ({ accounts: st.accounts.filter((x) => x.id !== id) })),

  markSyncing: (id) =>
    set((st) => {
      const next = new Set(st.syncingAccountIds);
      next.add(id);
      return { syncingAccountIds: next };
    }),

  markSynced: (id) =>
    set((st) => {
      const next = new Set(st.syncingAccountIds);
      next.delete(id);
      return { syncingAccountIds: next };
    }),

  setCalendars: (accountId, calendars) =>
    set((s) => {
      const next = new Map(s.accountCalendars);
      next.set(accountId, calendars);
      return { accountCalendars: next };
    }),
}));
