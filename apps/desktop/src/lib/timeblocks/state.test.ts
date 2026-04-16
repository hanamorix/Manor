import { describe, it, expect, beforeEach } from "vitest";
import { useTimeBlocksStore } from "./state";
import type { TimeBlock, PatternSuggestion } from "./ipc";

const sampleBlock = (overrides: Partial<TimeBlock> = {}): TimeBlock => ({
  id: 1,
  title: "Focus",
  kind: "focus",
  date: 1_776_259_200_000,
  start_time: "09:00",
  end_time: "11:00",
  rrule: null,
  is_pattern: false,
  pattern_nudge_dismissed_at: null,
  created_at: Date.now(),
  deleted_at: null,
  ...overrides,
});

const sampleSuggestion = (overrides: Partial<PatternSuggestion> = {}): PatternSuggestion => ({
  trigger_id: 1,
  kind: "focus",
  start_time: "09:00",
  end_time: "11:00",
  weekday: "Tuesday",
  count: 3,
  ...overrides,
});

describe("useTimeBlocksStore", () => {
  beforeEach(() => {
    useTimeBlocksStore.setState(useTimeBlocksStore.getInitialState(), true);
  });

  it("starts empty", () => {
    const s = useTimeBlocksStore.getState();
    expect(s.todayBlocks).toEqual([]);
    expect(s.weekBlocks).toEqual([]);
    expect(s.recurringBlocks).toEqual([]);
    expect(s.patternSuggestion).toBeNull();
  });

  it("setTodayBlocks replaces the list", () => {
    const a = sampleBlock({ id: 1 });
    const b = sampleBlock({ id: 2 });
    useTimeBlocksStore.getState().setTodayBlocks([a, b]);
    expect(useTimeBlocksStore.getState().todayBlocks).toEqual([a, b]);
  });

  it("upsertBlock appends a new id", () => {
    const a = sampleBlock({ id: 1 });
    const b = sampleBlock({ id: 2 });
    useTimeBlocksStore.getState().setTodayBlocks([a]);
    useTimeBlocksStore.getState().upsertBlock(b);
    expect(useTimeBlocksStore.getState().todayBlocks).toEqual([a, b]);
  });

  it("upsertBlock updates both today and week lists", () => {
    const a = sampleBlock({ id: 1, title: "Old" });
    useTimeBlocksStore.getState().setTodayBlocks([a]);
    useTimeBlocksStore.getState().setWeekBlocks([a]);
    useTimeBlocksStore.getState().upsertBlock({ ...a, title: "New" });
    expect(useTimeBlocksStore.getState().todayBlocks[0].title).toBe("New");
    expect(useTimeBlocksStore.getState().weekBlocks[0].title).toBe("New");
  });

  it("removeBlock strips it from all three lists", () => {
    const a = sampleBlock({ id: 1 });
    useTimeBlocksStore.getState().setTodayBlocks([a]);
    useTimeBlocksStore.getState().setWeekBlocks([a]);
    useTimeBlocksStore.getState().setRecurringBlocks([a]);
    useTimeBlocksStore.getState().removeBlock(1);
    expect(useTimeBlocksStore.getState().todayBlocks).toEqual([]);
    expect(useTimeBlocksStore.getState().weekBlocks).toEqual([]);
    expect(useTimeBlocksStore.getState().recurringBlocks).toEqual([]);
  });

  it("setPatternSuggestion stores and clears", () => {
    const s = sampleSuggestion();
    useTimeBlocksStore.getState().setPatternSuggestion(s);
    expect(useTimeBlocksStore.getState().patternSuggestion).toEqual(s);
    useTimeBlocksStore.getState().setPatternSuggestion(null);
    expect(useTimeBlocksStore.getState().patternSuggestion).toBeNull();
  });
});
