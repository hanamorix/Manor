import { create } from "zustand";
import type { Task, Proposal, Event } from "./ipc";

interface TodayStore {
  tasks: Task[];
  pendingProposals: Proposal[];
  events: Event[];
  toast: { message: string; expiresAt: number } | null;

  setTasks: (t: Task[]) => void;
  upsertTask: (t: Task) => void;
  removeTask: (id: number) => void;

  setPendingProposals: (p: Proposal[]) => void;
  removeProposal: (id: number) => void;

  setEvents: (e: Event[]) => void;

  showToast: (message: string) => void;
  clearToast: () => void;
}

export const useTodayStore = create<TodayStore>((set) => ({
  tasks: [],
  pendingProposals: [],
  events: [],
  toast: null,

  setTasks: (t) => set({ tasks: t }),

  upsertTask: (t) =>
    set((st) => {
      const idx = st.tasks.findIndex((x) => x.id === t.id);
      if (idx === -1) return { tasks: [...st.tasks, t] };
      const next = st.tasks.slice();
      next[idx] = t;
      return { tasks: next };
    }),

  removeTask: (id) =>
    set((st) => ({ tasks: st.tasks.filter((x) => x.id !== id) })),

  setPendingProposals: (p) => set({ pendingProposals: p }),

  removeProposal: (id) =>
    set((st) => ({ pendingProposals: st.pendingProposals.filter((x) => x.id !== id) })),

  setEvents: (e) => set({ events: e }),

  showToast: (message) =>
    set({ toast: { message, expiresAt: Date.now() + 2000 } }),

  clearToast: () => set({ toast: null }),
}));
