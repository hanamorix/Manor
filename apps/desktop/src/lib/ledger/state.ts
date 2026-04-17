import { create } from "zustand";
import type {
  Budget, Category, Contract, MonthlySummary,
  RecurringPayment, RenewalAlert, Transaction
} from "./ipc";

interface LedgerStore {
  categories: Category[];
  transactions: Transaction[];
  budgets: Budget[];
  summary: MonthlySummary | null;
  recurring: RecurringPayment[];
  contracts: Contract[];
  renewalAlerts: RenewalAlert[];
  currentYear: number;
  currentMonth: number;

  setCategories: (c: Category[]) => void;
  setTransactions: (t: Transaction[]) => void;
  setBudgets: (b: Budget[]) => void;
  setSummary: (s: MonthlySummary) => void;
  setRecurring: (r: RecurringPayment[]) => void;
  setContracts: (c: Contract[]) => void;
  setRenewalAlerts: (a: RenewalAlert[]) => void;
  upsertTransaction: (t: Transaction) => void;
  removeTransaction: (id: number) => void;
  upsertCategory: (c: Category) => void;
  removeCategory: (id: number) => void;
  upsertBudget: (b: Budget) => void;
  removeBudget: (id: number) => void;
  upsertRecurring: (r: RecurringPayment) => void;
  removeRecurring: (id: number) => void;
  upsertContract: (c: Contract) => void;
  removeContract: (id: number) => void;
}

const now = new Date();

export const useLedgerStore = create<LedgerStore>((set) => ({
  categories: [],
  transactions: [],
  budgets: [],
  summary: null,
  recurring: [],
  contracts: [],
  renewalAlerts: [],
  currentYear: now.getFullYear(),
  currentMonth: now.getMonth() + 1,

  setCategories: (c) => set({ categories: c }),
  setTransactions: (t) => set({ transactions: t }),
  setBudgets: (b) => set({ budgets: b }),
  setSummary: (s) => set({ summary: s }),
  setRecurring: (r) => set({ recurring: r }),
  setContracts: (c) => set({ contracts: c }),
  setRenewalAlerts: (a) => set({ renewalAlerts: a }),

  upsertTransaction: (t) =>
    set((st) => {
      const idx = st.transactions.findIndex((x) => x.id === t.id);
      if (idx === -1) return { transactions: [t, ...st.transactions] };
      const next = st.transactions.slice();
      next[idx] = t;
      return { transactions: next };
    }),
  removeTransaction: (id) =>
    set((st) => ({ transactions: st.transactions.filter((x) => x.id !== id) })),

  upsertCategory: (c) =>
    set((st) => {
      const idx = st.categories.findIndex((x) => x.id === c.id);
      if (idx === -1) return { categories: [...st.categories, c] };
      const next = st.categories.slice(); next[idx] = c;
      return { categories: next };
    }),
  removeCategory: (id) =>
    set((st) => ({ categories: st.categories.filter((x) => x.id !== id) })),

  upsertBudget: (b) =>
    set((st) => {
      const idx = st.budgets.findIndex((x) => x.id === b.id);
      if (idx === -1) return { budgets: [...st.budgets, b] };
      const next = st.budgets.slice(); next[idx] = b;
      return { budgets: next };
    }),
  removeBudget: (id) =>
    set((st) => ({ budgets: st.budgets.filter((x) => x.id !== id) })),

  upsertRecurring: (r) =>
    set((st) => {
      const idx = st.recurring.findIndex((x) => x.id === r.id);
      if (idx === -1) return { recurring: [...st.recurring, r] };
      const next = st.recurring.slice(); next[idx] = r;
      return { recurring: next };
    }),
  removeRecurring: (id) =>
    set((st) => ({ recurring: st.recurring.filter((x) => x.id !== id) })),

  upsertContract: (c) =>
    set((st) => {
      const idx = st.contracts.findIndex((x) => x.id === c.id);
      if (idx === -1) return { contracts: [...st.contracts, c] };
      const next = st.contracts.slice(); next[idx] = c;
      return { contracts: next };
    }),
  removeContract: (id) =>
    set((st) => ({ contracts: st.contracts.filter((x) => x.id !== id) })),
}));
