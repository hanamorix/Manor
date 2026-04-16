import { create } from "zustand";
import type { AssistantState } from "./expressions";
import type { Message } from "./ipc";

export interface TransientBubble {
  id: string;
  kind: "user" | "assistant" | "error";
  content: string;
  messageId: number | null; // the DB message id, for click-to-mark-seen / scroll-to
  ttlMs: number;
}

interface AssistantStore {
  avatarState: AssistantState;
  messages: Message[];
  transientBubbles: TransientBubble[];
  unreadCount: number;
  drawerOpen: boolean;

  setAvatarState: (s: AssistantState) => void;
  hydrateMessages: (msgs: Message[]) => void;
  beginAssistantMessage: (id: number) => void;
  appendAssistantToken: (fragment: string) => void;
  endAssistantMessage: () => void;
  addUserMessage: (msg: Message) => void;

  enqueueBubble: (b: TransientBubble) => void;
  appendBubbleContent: (id: string, fragment: string) => void;
  setBubbleTtl: (id: string, ttlMs: number) => void;
  dismissBubble: (id: string) => void;

  setUnreadCount: (n: number) => void;
  setDrawerOpen: (open: boolean) => void;
}

const MAX_VISIBLE_BUBBLES = 3;

export const useAssistantStore = create<AssistantStore>((set) => ({
  avatarState: "idle",
  messages: [],
  transientBubbles: [],
  unreadCount: 0,
  drawerOpen: false,

  setAvatarState: (s) => set({ avatarState: s }),

  hydrateMessages: (msgs) => set({ messages: msgs }),

  beginAssistantMessage: (id) =>
    set((st) => ({
      messages: [
        ...st.messages,
        {
          id,
          conversation_id: 1,
          role: "assistant",
          content: "",
          created_at: Date.now(),
          seen: false,
          proposal_id: null,
        },
      ],
    })),

  appendAssistantToken: (fragment) =>
    set((st) => {
      const last = st.messages[st.messages.length - 1];
      if (!last || last.role !== "assistant") return st;
      const updated = { ...last, content: last.content + fragment };
      return { messages: [...st.messages.slice(0, -1), updated] };
    }),

  endAssistantMessage: () => set({ avatarState: "idle" }),

  addUserMessage: (msg) =>
    set((st) => ({ messages: [...st.messages, msg] })),

  enqueueBubble: (b) =>
    set((st) => {
      const next = [...st.transientBubbles, b];
      while (next.length > MAX_VISIBLE_BUBBLES) next.shift();
      return { transientBubbles: next };
    }),

  appendBubbleContent: (id, fragment) =>
    set((st) => ({
      transientBubbles: st.transientBubbles.map((b) =>
        b.id === id ? { ...b, content: b.content + fragment } : b,
      ),
    })),

  setBubbleTtl: (id, ttlMs) =>
    set((st) => ({
      transientBubbles: st.transientBubbles.map((b) =>
        b.id === id ? { ...b, ttlMs } : b,
      ),
    })),

  dismissBubble: (id) =>
    set((st) => ({
      transientBubbles: st.transientBubbles.filter((b) => b.id !== id),
    })),

  setUnreadCount: (n) => set({ unreadCount: n }),

  setDrawerOpen: (open) => set({ drawerOpen: open }),
}));
