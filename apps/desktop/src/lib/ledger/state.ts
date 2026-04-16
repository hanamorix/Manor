import { create } from "zustand";
import type { Budget, Category, MonthlySummary, Transaction } from "./ipc";

interface LedgerStore {
  categories: Category[];
  transactions: Transaction[];
  budgets: Budget[];
  summary: MonthlySummary | null;
  currentYear: number;
  currentMonth: number;

  setCategories: (c: Category[]) => void;
  setTransactions: (t: Transaction[]) => void;
  setBudgets: (b: Budget[]) => void;
  setSummary: (s: MonthlySummary) => void;
  upsertTransaction: (t: Transaction) => void;
  removeTransaction: (id: number) => void;
  upsertCategory: (c: Category) => void;
  removeCategory: (id: number) => void;
  upsertBudget: (b: Budget) => void;
  removeBudget: (id: number) => void;
}

const now = new Date();

export const useLedgerStore = create<LedgerStore>((set) => ({
  categories: [],
  transactions: [],
  budgets: [],
  summary: null,
  currentYear: now.getFullYear(),
  currentMonth: now.getMonth() + 1,

  setCategories: (c) => set({ categories: c }),
  setTransactions: (t) => set({ transactions: t }),
  setBudgets: (b) => set({ budgets: b }),
  setSummary: (s) => set({ summary: s }),

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
      const next = st.categories.slice();
      next[idx] = c;
      return { categories: next };
    }),

  removeCategory: (id) =>
    set((st) => ({ categories: st.categories.filter((x) => x.id !== id) })),

  upsertBudget: (b) =>
    set((st) => {
      const idx = st.budgets.findIndex((x) => x.id === b.id);
      if (idx === -1) return { budgets: [...st.budgets, b] };
      const next = st.budgets.slice();
      next[idx] = b;
      return { budgets: next };
    }),

  removeBudget: (id) =>
    set((st) => ({ budgets: st.budgets.filter((x) => x.id !== id) })),
}));
