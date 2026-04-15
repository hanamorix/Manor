import { useEffect, useRef } from "react";
import Avatar from "./Avatar";
import BubbleLayer from "./BubbleLayer";
import InputPill from "./InputPill";
import UnreadBadge from "./UnreadBadge";
import ConversationDrawer from "./ConversationDrawer";
import { useAssistantStore } from "../../lib/assistant/state";
import { sendMessage, getUnreadCount, listMessages } from "../../lib/assistant/ipc";
import type { StreamChunk } from "../../lib/assistant/ipc";

function newBubbleId() {
  return Math.random().toString(36).slice(2, 10);
}

function looksLikeDelight(content: string): boolean {
  if (/[\u{1F389}\u{1F38A}]/u.test(content)) return true;
  const exclaims = (content.match(/!/g) || []).length;
  return exclaims >= 3;
}

export default function Assistant() {
  const pillRef = useRef<HTMLInputElement>(null);

  const setAvatarState = useAssistantStore((s) => s.setAvatarState);
  const enqueueBubble = useAssistantStore((s) => s.enqueueBubble);
  const appendBubbleContent = useAssistantStore((s) => s.appendBubbleContent);
  const transientBubbles = useAssistantStore((s) => s.transientBubbles);
  const beginAssistantMessage = useAssistantStore((s) => s.beginAssistantMessage);
  const appendAssistantToken = useAssistantStore((s) => s.appendAssistantToken);
  const endAssistantMessage = useAssistantStore((s) => s.endAssistantMessage);
  const addUserMessage = useAssistantStore((s) => s.addUserMessage);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);
  const setDrawerOpen = useAssistantStore((s) => s.setDrawerOpen);
  const hydrateMessages = useAssistantStore((s) => s.hydrateMessages);

  // Initial load: hydrate recent messages + unread count.
  useEffect(() => {
    void (async () => {
      const msgs = await listMessages(100, 0);
      hydrateMessages(msgs);
      const n = await getUnreadCount();
      setUnreadCount(n);
    })();
  }, [hydrateMessages, setUnreadCount]);

  // Global ⌘/ — focus the pill (only when Manor has window focus; listener is scoped to document).
  useEffect(() => {
    let lastFire = 0;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        const now = Date.now();
        if (now - lastFire < 150) return;
        lastFire = now;
        e.preventDefault();
        pillRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const handleSubmit = async (content: string) => {
    setAvatarState("listening");

    // Optimistic: add a blue user bubble + a provisional message to the scrollback.
    const userBubbleId = newBubbleId();
    enqueueBubble({
      id: userBubbleId,
      kind: "user",
      content,
      messageId: null,
      ttlMs: 6000,
    });
    // We don't know the DB id yet, but addUserMessage takes a full Message shape.
    // Use a negative temporary id that won't collide with real ids; the drawer
    // re-hydrates from the DB on open, so this is fine.
    addUserMessage({
      id: -Date.now(),
      conversation_id: 1,
      role: "user",
      content,
      created_at: Date.now(),
      seen: true,
      proposal_id: null,
    });

    setAvatarState("thinking");

    let assistantDbId: number | null = null;
    const assistantBubbleId = newBubbleId();
    let assistantText = "";

    const onEvent = (chunk: StreamChunk) => {
      if (chunk.type === "Started") {
        assistantDbId = chunk.value;
        beginAssistantMessage(assistantDbId);
        setAvatarState("speaking");
        enqueueBubble({
          id: assistantBubbleId,
          kind: "assistant",
          content: "",
          messageId: assistantDbId,
          ttlMs: 12000,
        });
      } else if (chunk.type === "Token") {
        if (assistantDbId === null) {
          // Defensive — Started should always fire first. If not, we still want
          // tokens to appear, so synthesize and continue.
          assistantDbId = -Date.now();
          beginAssistantMessage(assistantDbId);
          setAvatarState("speaking");
          enqueueBubble({
            id: assistantBubbleId,
            kind: "assistant",
            content: "",
            messageId: assistantDbId,
            ttlMs: 12000,
          });
        }
        assistantText += chunk.value;
        appendAssistantToken(chunk.value);
        appendBubbleContent(assistantBubbleId, chunk.value);
      } else if (chunk.type === "Done") {
        endAssistantMessage();
        if (looksLikeDelight(assistantText)) {
          setAvatarState("idle"); // will pass through laughing in a future refinement
        } else {
          setAvatarState("idle");
        }
        // Refresh unread count from DB — the authoritative source.
        void getUnreadCount().then(setUnreadCount);
      } else if (chunk.type === "Error") {
        setAvatarState("confused");
        const errorMessage =
          chunk.value === "OllamaUnreachable"
            ? "I can't reach Ollama. Is it running?"
            : chunk.value === "ModelMissing"
              ? "I need the model `qwen2.5:7b-instruct`. Run `./scripts/install-ollama.sh`."
              : chunk.value === "Interrupted"
                ? "The reply was interrupted — check Ollama."
                : "Something went wrong. Check the logs.";
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: errorMessage,
          messageId: null,
          ttlMs: 12000,
        });
      }
    };

    try {
      await sendMessage(content, onEvent);
    } catch (e) {
      setAvatarState("confused");
      enqueueBubble({
        id: newBubbleId(),
        kind: "error",
        content: `IPC error: ${String(e)}`,
        messageId: null,
        ttlMs: 7000,
      });
    }
  };

  return (
    <>
      <ConversationDrawer onSubmit={handleSubmit} />
      <BubbleLayer />

      <div
        style={{
          position: "fixed",
          bottom: 16,
          right: 16,
          display: "flex",
          flexDirection: "column",
          alignItems: "flex-end",
          gap: 8,
          zIndex: 1000,
        }}
      >
        <UnreadBadgeWithAnchor />
        {transientBubbles.length === 0 && (
          <InputPill
            ref={pillRef}
            onSubmit={handleSubmit}
            onFocus={() => setAvatarState("listening")}
            onBlur={() => setAvatarState("idle")}
          />
        )}
        <Avatar onClick={() => setDrawerOpen(true)} />
      </div>
    </>
  );
}

function UnreadBadgeWithAnchor() {
  return (
    <div style={{ position: "relative" }}>
      <div style={{ position: "absolute", top: -6, right: -6 }}>
        <UnreadBadge />
      </div>
    </div>
  );
}
