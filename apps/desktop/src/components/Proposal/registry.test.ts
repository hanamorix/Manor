import { describe, it, expect } from "vitest";
import {
  PROPOSAL_KIND_HANDLERS,
  getProposalHandler,
  type AddTaskParsed,
  type AddEventParsed,
  type CompleteTaskParsed,
  type AddChoreParsed,
  type AddTimeBlockParsed,
  type CompleteChoreParsed,
  type AddMaintenanceScheduleParsed,
} from "./registry";

describe("PROPOSAL_KIND_HANDLERS", () => {
  it("contains the Phase 1 entries plus the Phase 2 add_chore entry", () => {
    expect(Object.keys(PROPOSAL_KIND_HANDLERS).sort()).toEqual([
      "add_chore",
      "add_event",
      "add_maintenance_schedule",
      "add_recurring_block",
      "add_task",
      "add_time_block",
      "complete_chore",
      "complete_task",
    ]);
  });
});

describe("getProposalHandler", () => {
  it("returns null for unknown kinds", () => {
    expect(getProposalHandler("definitely_not_a_kind")).toBeNull();
  });

  it("returns the add_task handler", () => {
    expect(getProposalHandler("add_task")).not.toBeNull();
  });
});

describe("add_task handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_task;

  it("parses diff JSON into AddTaskParsed", () => {
    const parsed = handler.parse(
      JSON.stringify({ title: "Buy milk" }),
    ) as AddTaskParsed;
    expect(parsed.title).toBe("Buy milk");
  });

  it("round-trips title + due_date through parse", () => {
    const parsed = handler.parse(
      JSON.stringify({ title: "Email landlord", due_date: "2026-05-15" }),
    ) as AddTaskParsed;
    expect(parsed.title).toBe("Email landlord");
    expect(parsed.due_date).toBe("2026-05-15");
  });

  it("summarises without due_date", () => {
    const out = handler.summarise({ title: "Buy milk" });
    expect(out).toBe("Add task: Buy milk");
  });

  it("summarises with due_date suffix", () => {
    const out = handler.summarise({
      title: "Email landlord",
      due_date: "2026-05-15",
    });
    expect(out).toBe("Add task: Email landlord (due 2026-05-15)");
  });

  it("does not declare supportsEdit in Phase 1", () => {
    expect(handler.supportsEdit).toBeFalsy();
  });
});

describe("complete_task handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.complete_task;

  it("parses task title completion diffs", () => {
    const parsed = handler.parse(
      JSON.stringify({ title: "Buy milk" }),
    ) as CompleteTaskParsed;
    expect(parsed.title).toBe("Buy milk");
  });

  it("summarises title and id targets", () => {
    expect(handler.summarise({ title: "Buy milk" })).toBe(
      "Complete task: Buy milk",
    );
    expect(handler.summarise({ task_id: 12 })).toBe("Complete task: #12");
  });
});

describe("add_event handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_event;

  it("parses a single event diff into a one-item array", () => {
    const parsed = handler.parse(
      JSON.stringify({
        title: "Dentist",
        start_at: 1778842800,
        end_at: 1778846400,
      }),
    ) as AddEventParsed[];
    expect(parsed).toHaveLength(1);
    expect(parsed[0].title).toBe("Dentist");
  });

  it("summarises event bundles", () => {
    expect(
      handler.summarise([
        { title: "Dentist", start_at: 1778842800, end_at: 1778846400 },
        { title: "Lunch", start_at: 1778850000, end_at: 1778853600 },
      ]),
    ).toBe("Add 2 events");
  });
});

describe("add_chore handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_chore;

  it("parses a single diff JSON object into a one-item array", () => {
    const parsed = handler.parse(
      JSON.stringify({ title: "Do dishes", rrule: "FREQ=DAILY" }),
    ) as AddChoreParsed[];
    expect(parsed).toHaveLength(1);
    expect(parsed[0].title).toBe("Do dishes");
  });

  it("parses bundle diff JSON unchanged", () => {
    const parsed = handler.parse(
      JSON.stringify([
        { title: "Bins", rrule: "weekly" },
        { title: "Laundry", rrule: "weekly" },
      ]),
    ) as AddChoreParsed[];
    expect(parsed.map((p) => p.title)).toEqual(["Bins", "Laundry"]);
  });

  it("summarises a single chore", () => {
    expect(handler.summarise([{ title: "Bins", rrule: "weekly" }])).toBe(
      "Add chore: Bins",
    );
  });

  it("summarises a chore bundle", () => {
    expect(
      handler.summarise([
        { title: "Bins", rrule: "weekly" },
        { title: "Laundry", rrule: "weekly" },
      ]),
    ).toBe("Add 2 chores");
  });
});

describe("complete_chore handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.complete_chore;

  it("parses chore title completion diffs", () => {
    const parsed = handler.parse(
      JSON.stringify({ title: "Do dishes" }),
    ) as CompleteChoreParsed;
    expect(parsed.title).toBe("Do dishes");
  });

  it("summarises title and id targets", () => {
    expect(handler.summarise({ title: "Bins" })).toBe("Complete chore: Bins");
    expect(handler.summarise({ chore_id: 12 })).toBe("Complete chore: #12");
  });
});

describe("time block handlers", () => {
  it("summarises one-off blocks", () => {
    const handler = PROPOSAL_KIND_HANDLERS.add_time_block;
    const parsed = handler.parse(
      JSON.stringify({
        title: "Deep work",
        kind: "focus",
        date_ms: 1777132800000,
        start_time: "09:00",
        end_time: "11:00",
      }),
    ) as AddTimeBlockParsed;
    expect(handler.summarise(parsed)).toBe("Add block: Deep work (09:00-11:00)");
  });

  it("summarises recurring blocks", () => {
    const handler = PROPOSAL_KIND_HANDLERS.add_recurring_block;
    expect(
      handler.summarise({
        title: "Weekly planning",
        kind: "admin",
        date_ms: 1777132800000,
        start_time: "09:00",
        end_time: "09:30",
        rrule: "FREQ=WEEKLY;BYDAY=MO",
      }),
    ).toBe("Add recurring block: Weekly planning (09:00-09:30)");
  });
});

describe("add_maintenance_schedule handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_maintenance_schedule;

  const fixture: AddMaintenanceScheduleParsed = {
    asset_id: "asset-uuid-1",
    task: "Annual service",
    interval_months: 12,
    notes: "Service notes from PDF",
    source_attachment_uuid: "att-1",
    tier: "ollama",
  };

  it("parses diff JSON into AddMaintenanceScheduleParsed", () => {
    const parsed = handler.parse(
      JSON.stringify(fixture),
    ) as AddMaintenanceScheduleParsed;
    expect(parsed).toEqual(fixture);
  });

  it("summarises with task and interval (singular)", () => {
    const out = handler.summarise({ ...fixture, interval_months: 1 });
    expect(out).toBe("Annual service · every 1 month");
  });

  it("summarises with task and interval (plural)", () => {
    const out = handler.summarise(fixture);
    expect(out).toBe("Annual service · every 12 months");
  });

  it("declares supportsEdit and provides an EditDrawer", () => {
    expect(handler.supportsEdit).toBe(true);
    expect(handler.EditDrawer).toBeDefined();
  });
});
