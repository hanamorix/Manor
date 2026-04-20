import { useEffect, useMemo, useRef, useState } from "react";
import Avatar from "./Avatar";
import ChatDock from "./ChatDock";
import ChatHistoryPanel from "./ChatHistoryPanel";
import { EphemeralLog, type Exchange } from "./EphemeralLog";
import UnreadBadge from "./UnreadBadge";
import { useAssistantStore } from "../../lib/assistant/state";
import { sendMessage, getUnreadCount, listMessages, markSeen } from "../../lib/assistant/ipc";
import type { StreamChunk, Message as AssistantMessage } from "../../lib/assistant/ipc";
import { parseSlash } from "../../lib/today/slash";
import { addTask, listTasks, listProposals } from "../../lib/today/ipc";
import { addTransaction } from "../../lib/ledger/ipc";
import { useTodayStore } from "../../lib/today/state";

function newBubbleId() {
  return Math.random().toString(36).slice(2, 10);
}

const MENU_WIDTH_PX = 70;
const AVATAR_COLUMN_PX = 104;
const EPHEMERAL_FADE_MS = 10000;

export default function Assistant() {
  const dockRef = useRef<HTMLInputElement>(null);

  const enqueueBubble = useAssistantStore((s) => s.enqueueBubble);
  const appendBubbleContent = useAssistantStore((s) => s.appendBubbleContent);
  const beginAssistantMessage = useAssistantStore((s) => s.beginAssistantMessage);
  const appendAssistantToken = useAssistantStore((s) => s.appendAssistantToken);
  const endAssistantMessage = useAssistantStore((s) => s.endAssistantMessage);
  const addUserMessage = useAssistantStore((s) => s.addUserMessage);
  const setBubbleTtl = useAssistantStore((s) => s.setBubbleTtl);
  const setUnreadCount = useAssistantStore((s) => s.setUnreadCount);
  const hydrateMessages = useAssistantStore((s) => s.hydrateMessages);
  const messages = useAssistantStore((s) => s.messages);

  const markAllLocalSeen = useAssistantStore((s) => s.markAllLocalSeen);

  const setTodayTasks = useTodayStore((s) => s.setTasks);
  const setPendingProposals = useTodayStore((s) => s.setPendingProposals);

  const [isHistoryOpen, setIsHistoryOpen] = useState(false);
  const [ephemeralVisible, setEphemeralVisible] = useState(false);
  const ephemeralTimerRef = useRef<number | null>(null);

  const clearEphemeralTimer = () => {
    if (ephemeralTimerRef.current !== null) {
      window.clearTimeout(ephemeralTimerRef.current);
      ephemeralTimerRef.current = null;
    }
  };

  useEffect(() => () => clearEphemeralTimer(), []);

  useEffect(() => {
    void (async () => {
      const msgs = await listMessages(100, 0);
      hydrateMessages(msgs);
      const n = await getUnreadCount();
      setUnreadCount(n);
    })();
  }, [hydrateMessages, setUnreadCount]);

  useEffect(() => {
    let lastFire = 0;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        const now = Date.now();
        if (now - lastFire < 150) return;
        lastFire = now;
        e.preventDefault();
        dockRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!isHistoryOpen) return;
    dockRef.current?.focus();
    clearEphemeralTimer();
    setEphemeralVisible(false);
    const unseenIds = messages
      .filter((m) => !m.seen && m.id > 0)
      .map((m) => m.id);
    if (unseenIds.length === 0) return;
    void markSeen(unseenIds).then(() => {
      markAllLocalSeen();
      setUnreadCount(0);
    });
  }, [isHistoryOpen, messages, markAllLocalSeen, setUnreadCount]);

  const lastTwoExchanges = useMemo<Exchange[]>(() => {
    return extractExchanges(messages).slice(-2).reverse();
  }, [messages]);

  const handleSubmit = async (content: string) => {
    const slash = parseSlash(content);
    if (slash?.type === "task") {
      try {
        const task = await addTask(slash.title);
        useTodayStore.getState().upsertTask(task);
        useTodayStore.getState().showToast(`Added: ${slash.title}`);
        return;
      } catch (e) {
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: `Couldn't add task: ${String(e)}`,
          messageId: null,
          ttlMs: 7000,
        });
        return;
      }
    }
    if (slash?.type === "spent") {
      try {
        const now = new Date();
        now.setHours(0, 0, 0, 0);
        await addTransaction({
          amountPence: slash.amountPence,
          currency: "GBP",
          description: slash.description,
          date: Math.floor(now.getTime() / 1000),
        });
        enqueueBubble({
          id: newBubbleId(),
          kind: "assistant",
          content: `Added: ${slash.description} (£${(Math.abs(slash.amountPence) / 100).toFixed(2)})`,
          messageId: null,
          ttlMs: 6000,
        });
        return;
      } catch (e) {
        enqueueBubble({
          id: newBubbleId(),
          kind: "error",
          content: `Couldn't add transaction: ${String(e)}`,
          messageId: null,
          ttlMs: 7000,
        });
        return;
      }
    }

    const userBubbleId = newBubbleId();
    enqueueBubble({
      id: userBubbleId,
      kind: "user",
      content,
      messageId: null,
      ttlMs: 10000,
    });
    addUserMessage({
      id: -Date.now(),
      conversation_id: 1,
      role: "user",
      content,
      created_at: Date.now(),
      seen: true,
      proposal_id: null,
    });

    let assistantDbId: number | null = null;
    const assistantBubbleId = newBubbleId();

    const onEvent = (chunk: StreamChunk) => {
      if (chunk.type === "Started") {
        assistantDbId = chunk.value;
        beginAssistantMessage(assistantDbId);
        clearEphemeralTimer();
        setEphemeralVisible(true);
        enqueueBubble({
          id: assistantBubbleId,
          kind: "assistant",
          content: "",
          messageId: assistantDbId,
          ttlMs: 120000,
        });
      } else if (chunk.type === "Token") {
        if (assistantDbId === null) {
          assistantDbId = -Date.now();
          beginAssistantMessage(assistantDbId);
          clearEphemeralTimer();
          setEphemeralVisible(true);
          enqueueBubble({
            id: assistantBubbleId,
            kind: "assistant",
            content: "",
            messageId: assistantDbId,
            ttlMs: 12000,
          });
        }
        appendAssistantToken(chunk.value);
        appendBubbleContent(assistantBubbleId, chunk.value);
      } else if (chunk.type === "Proposal") {
        void listProposals("pending").then(setPendingProposals);
        void listTasks().then(setTodayTasks);
      } else if (chunk.type === "Done") {
        endAssistantMessage();
        setBubbleTtl(assistantBubbleId, 8000);
        void getUnreadCount().then(setUnreadCount);
        clearEphemeralTimer();
        ephemeralTimerRef.current = window.setTimeout(() => {
          setEphemeralVisible(false);
          ephemeralTimerRef.current = null;
        }, EPHEMERAL_FADE_MS);
      } else if (chunk.type === "Error") {
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
      <div
        style={{
          position: "fixed",
          left: MENU_WIDTH_PX + 16,
          right: AVATAR_COLUMN_PX,
          bottom: 16,
          zIndex: 999,
        }}
      >
        {!isHistoryOpen && (
          <EphemeralLog
            exchanges={lastTwoExchanges}
            onExpand={() => setIsHistoryOpen(true)}
            visible={ephemeralVisible}
          />
        )}
        <ChatHistoryPanel
          isOpen={isHistoryOpen}
          messages={messages}
          onCollapse={() => setIsHistoryOpen(false)}
        />
        <ChatDock
          ref={dockRef}
          onSubmit={handleSubmit}
          onExpand={() => setIsHistoryOpen(true)}
        />
      </div>

      <div
        style={{
          position: "fixed",
          bottom: 0,
          right: 16,
          zIndex: 1000,
        }}
      >
        <div style={{ position: "relative", display: "inline-block" }}>
          <Avatar height={72} onClick={() => setIsHistoryOpen((v) => !v)} />
          <div
            style={{
              position: "absolute",
              top: -6,
              right: -6,
              zIndex: 2,
            }}
          >
            <UnreadBadge />
          </div>
        </div>
      </div>
    </>
  );
}

function extractExchanges(messages: AssistantMessage[]): Exchange[] {
  const out: Exchange[] = [];
  for (let i = 0; i < messages.length - 1; i++) {
    const a = messages[i];
    const b = messages[i + 1];
    if (a.role === "user" && b.role === "assistant") {
      out.push({ userText: a.content, assistantText: b.content, key: b.id });
      i++;
    }
  }
  return out;
}

