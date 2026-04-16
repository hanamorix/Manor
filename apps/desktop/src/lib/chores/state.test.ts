import { describe, it, expect, beforeEach } from "vitest";
import { useChoresStore } from "./state";
import type { Chore, FairnessNudge } from "./ipc";

const sampleChore = (overrides: Partial<Chore> = {}): Chore => ({
  id: 1,
  title: "Bins",
  emoji: "🗑️",
  rrule: "FREQ=WEEKLY",
  next_due: Date.now(),
  rotation: "none",
  active: true,
  created_at: Date.now(),
  deleted_at: null,
  ...overrides,
});

const sampleNudge = (overrides: Partial<FairnessNudge> = {}): FairnessNudge => ({
  chore_id: 1,
  chore_title: "Bins",
  person_id: 1,
  person_name: "Rosa",
  days_ago: 21,
  ...overrides,
});

describe("useChoresStore", () => {
  beforeEach(() => {
    useChoresStore.setState(useChoresStore.getInitialState(), true);
  });

  it("starts empty", () => {
    const s = useChoresStore.getState();
    expect(s.choresDueToday).toEqual([]);
    expect(s.allChores).toEqual([]);
    expect(s.fairnessNudges).toEqual([]);
  });

  it("setChoresDueToday replaces the list", () => {
    const a = sampleChore({ id: 1, title: "A" });
    const b = sampleChore({ id: 2, title: "B" });
    useChoresStore.getState().setChoresDueToday([a, b]);
    expect(useChoresStore.getState().choresDueToday).toEqual([a, b]);
  });

  it("upsertChore appends a new id to allChores", () => {
    const a = sampleChore({ id: 1 });
    const b = sampleChore({ id: 2 });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().upsertChore(b);
    expect(useChoresStore.getState().allChores).toEqual([a, b]);
  });

  it("upsertChore updates an existing row", () => {
    const a = sampleChore({ id: 1, title: "Old" });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().upsertChore({ ...a, title: "New" });
    expect(useChoresStore.getState().allChores[0].title).toBe("New");
  });

  it("removeChore strips it from both lists", () => {
    const a = sampleChore({ id: 1 });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().setChoresDueToday([a]);
    useChoresStore.getState().removeChore(1);
    expect(useChoresStore.getState().allChores).toEqual([]);
    expect(useChoresStore.getState().choresDueToday).toEqual([]);
  });

  it("removeFromDueToday only touches the today list", () => {
    const a = sampleChore({ id: 1 });
    useChoresStore.getState().setAllChores([a]);
    useChoresStore.getState().setChoresDueToday([a]);
    useChoresStore.getState().removeFromDueToday(1);
    expect(useChoresStore.getState().allChores).toEqual([a]);
    expect(useChoresStore.getState().choresDueToday).toEqual([]);
  });

  it("dismissFairnessNudge drops the entry for that chore_id", () => {
    const n1 = sampleNudge({ chore_id: 1 });
    const n2 = sampleNudge({ chore_id: 2 });
    useChoresStore.getState().setFairnessNudges([n1, n2]);
    useChoresStore.getState().dismissFairnessNudge(1);
    expect(useChoresStore.getState().fairnessNudges).toEqual([n2]);
  });
});
