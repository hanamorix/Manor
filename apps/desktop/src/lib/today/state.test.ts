import { describe, it, expect, beforeEach } from "vitest";
import { useTodayStore } from "./state";
import type { Task, Proposal } from "./ipc";

const sampleTask = (overrides: Partial<Task> = {}): Task => ({
  id: 1,
  title: "Sample",
  due_date: "2026-04-15",
  completed_at: null,
  created_at: Date.now(),
  proposal_id: null,
  ...overrides,
});

const sampleProposal = (overrides: Partial<Proposal> = {}): Proposal => ({
  id: 1,
  kind: "add_task",
  rationale: "Manor said so",
  diff: '{"title":"X"}',
  status: "pending",
  proposed_at: Date.now(),
  applied_at: null,
  skill: "tasks",
  ...overrides,
});

describe("useTodayStore", () => {
  beforeEach(() => {
    useTodayStore.setState(useTodayStore.getInitialState(), true);
  });

  it("starts empty", () => {
    const s = useTodayStore.getState();
    expect(s.tasks).toEqual([]);
    expect(s.pendingProposals).toEqual([]);
  });

  it("setTasks replaces the array", () => {
    const a = sampleTask({ id: 1, title: "A" });
    const b = sampleTask({ id: 2, title: "B" });
    useTodayStore.getState().setTasks([a, b]);
    expect(useTodayStore.getState().tasks).toEqual([a, b]);
  });

  it("upsertTask appends a new id", () => {
    const a = sampleTask({ id: 1 });
    const b = sampleTask({ id: 2 });
    useTodayStore.getState().setTasks([a]);
    useTodayStore.getState().upsertTask(b);
    expect(useTodayStore.getState().tasks).toEqual([a, b]);
  });

  it("upsertTask replaces an existing id", () => {
    const a = sampleTask({ id: 1, title: "old" });
    const aPrime = sampleTask({ id: 1, title: "new" });
    useTodayStore.getState().setTasks([a]);
    useTodayStore.getState().upsertTask(aPrime);
    expect(useTodayStore.getState().tasks).toEqual([aPrime]);
  });

  it("removeTask drops by id", () => {
    const a = sampleTask({ id: 1 });
    const b = sampleTask({ id: 2 });
    useTodayStore.getState().setTasks([a, b]);
    useTodayStore.getState().removeTask(1);
    expect(useTodayStore.getState().tasks).toEqual([b]);
  });

  it("setPendingProposals replaces the array", () => {
    const p = sampleProposal();
    useTodayStore.getState().setPendingProposals([p]);
    expect(useTodayStore.getState().pendingProposals).toEqual([p]);
  });

  it("removeProposal drops by id", () => {
    const p1 = sampleProposal({ id: 1 });
    const p2 = sampleProposal({ id: 2 });
    useTodayStore.getState().setPendingProposals([p1, p2]);
    useTodayStore.getState().removeProposal(1);
    expect(useTodayStore.getState().pendingProposals).toEqual([p2]);
  });
});
