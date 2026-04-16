import { create } from "zustand";
import type { Task, Proposal, Event } from "./ipc";

interface TodayStore {
  tasks: Task[];
  events: Event[];
  pendingProposals: Proposal[];
  toast: { message: string; expiresAt: number } | null;

  setTasks: (t: Task[]) => void;
  setEvents: (e: Event[]) => void;
  upsertTask: (t: Task) => void;
  removeTask: (id: number) => void;
  upsertEvent: (e: Event) => void;
  removeEvent: (id: number) => void;

  setPendingProposals: (p: Proposal[]) => void;
  removeProposal: (id: number) => void;

  showToast: (message: string) => void;
  clearToast: () => void;
}

export const useTodayStore = create<TodayStore>((set) => ({
  tasks: [],
  events: [],
  pendingProposals: [],
  toast: null,

  setTasks: (t) => set({ tasks: t }),

  setEvents: (e) => set({ events: e }),

  upsertEvent: (e) =>
    set((s) => {
      const idx = s.events.findIndex((x) => x.id === e.id);
      if (idx >= 0) {
        const next = [...s.events];
        next[idx] = e;
        return { events: next };
      }
      return { events: [...s.events, e].sort((a, b) => a.start_at - b.start_at) };
    }),

  removeEvent: (id) => set((s) => ({ events: s.events.filter((e) => e.id !== id) })),

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

  showToast: (message) =>
    set({ toast: { message, expiresAt: Date.now() + 2000 } }),

  clearToast: () => set({ toast: null }),
}));
