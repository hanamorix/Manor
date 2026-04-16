import { invoke } from "@tauri-apps/api/core";

export type BlockKind = "focus" | "errands" | "admin" | "dnd";

export interface TimeBlock {
  id: number;
  title: string;
  kind: BlockKind;
  date: number;
  start_time: string;
  end_time: string;
  rrule: string | null;
  is_pattern: boolean;
  pattern_nudge_dismissed_at: number | null;
  created_at: number;
  deleted_at: number | null;
}

export interface PatternSuggestion {
  trigger_id: number;
  kind: string;
  start_time: string;
  end_time: string;
  weekday: string;
  count: number;
}

export interface CreateBlockResult {
  block: TimeBlock;
  suggestion: PatternSuggestion | null;
}

export async function listBlocksToday(): Promise<TimeBlock[]> {
  return invoke<TimeBlock[]>("list_blocks_today");
}

export async function listBlocksForWeek(weekStartMs: number): Promise<TimeBlock[]> {
  return invoke<TimeBlock[]>("list_blocks_for_week", { weekStartMs });
}

export async function listRecurringBlocks(): Promise<TimeBlock[]> {
  return invoke<TimeBlock[]>("list_recurring_blocks");
}

export async function createTimeBlock(args: {
  title: string;
  kind: BlockKind;
  dateMs: number;
  startTime: string;
  endTime: string;
}): Promise<CreateBlockResult> {
  return invoke<CreateBlockResult>("create_time_block", { args });
}

export async function updateTimeBlock(args: {
  id: number;
  title: string;
  kind: BlockKind;
  dateMs: number;
  startTime: string;
  endTime: string;
}): Promise<TimeBlock> {
  return invoke<TimeBlock>("update_time_block", { args });
}

export async function deleteTimeBlock(id: number): Promise<void> {
  return invoke<void>("delete_time_block", { id });
}

export async function promoteToPattern(id: number, rrule: string): Promise<TimeBlock> {
  return invoke<TimeBlock>("promote_to_pattern", { args: { id, rrule } });
}

export async function dismissPatternNudge(id: number): Promise<void> {
  return invoke<void>("dismiss_pattern_nudge", { id });
}

export async function checkTimeBlockPattern(id: number): Promise<PatternSuggestion | null> {
  return invoke<PatternSuggestion | null>("check_time_block_pattern", { id });
}
