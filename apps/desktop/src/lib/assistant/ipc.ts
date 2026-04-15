import { invoke, Channel } from "@tauri-apps/api/core";

export type Role = "user" | "assistant" | "system";

export interface Message {
  id: number;
  conversation_id: number;
  role: Role;
  content: string;
  created_at: number;
  seen: boolean;
  proposal_id: number | null;
}

export type StreamChunk =
  | { type: "Started"; value: number }
  | { type: "Token"; value: string }
  | { type: "Done" }
  | { type: "Error"; value: "OllamaUnreachable" | "ModelMissing" | "Interrupted" | "Unknown" };

export async function sendMessage(
  content: string,
  onEvent: (chunk: StreamChunk) => void,
): Promise<void> {
  const channel = new Channel<StreamChunk>();
  channel.onmessage = onEvent;
  return invoke<void>("send_message", { content, onEvent: channel });
}

export async function listMessages(limit = 100, offset = 0): Promise<Message[]> {
  return invoke<Message[]>("list_messages", { limit, offset });
}

export async function getUnreadCount(): Promise<number> {
  return invoke<number>("get_unread_count");
}

export async function markSeen(messageIds: number[]): Promise<void> {
  return invoke<void>("mark_seen", { messageIds });
}
