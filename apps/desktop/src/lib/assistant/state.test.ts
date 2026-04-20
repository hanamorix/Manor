import { describe, expect, it, beforeEach } from "vitest";
import { useAssistantStore } from "./state";

describe("assistant store", () => {
  beforeEach(() => {
    useAssistantStore.setState(useAssistantStore.getInitialState(), true);
  });

  it("starts with no messages, no bubbles, and zero unread", () => {
    const s = useAssistantStore.getState();
    expect(s.messages).toEqual([]);
    expect(s.transientBubbles).toEqual([]);
    expect(s.unreadCount).toBe(0);
  });

  it("appends assistant token fragments to the in-flight message", () => {
    const s = useAssistantStore.getState();
    s.beginAssistantMessage(42);
    s.appendAssistantToken("Hel");
    s.appendAssistantToken("lo.");
    s.endAssistantMessage();
    const msgs = useAssistantStore.getState().messages;
    expect(msgs).toHaveLength(1);
    expect(msgs[0]).toMatchObject({ id: 42, content: "Hello.", role: "assistant" });
  });

  it("enqueues a transient bubble and removes it by id", () => {
    const s = useAssistantStore.getState();
    s.enqueueBubble({
      id: "a",
      kind: "user",
      content: "hi",
      messageId: null,
      ttlMs: 3000,
    });
    expect(useAssistantStore.getState().transientBubbles).toHaveLength(1);
    s.dismissBubble("a");
    expect(useAssistantStore.getState().transientBubbles).toHaveLength(0);
  });

  it("caps visible bubbles at 3 — enqueueing a 4th evicts the oldest", () => {
    const s = useAssistantStore.getState();
    for (const id of ["a", "b", "c", "d"]) {
      s.enqueueBubble({ id, kind: "user", content: id, messageId: null, ttlMs: 3000 });
    }
    const ids = useAssistantStore.getState().transientBubbles.map((b) => b.id);
    expect(ids).toEqual(["b", "c", "d"]);
  });
});
