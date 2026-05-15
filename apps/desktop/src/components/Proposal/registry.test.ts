import { describe, it, expect } from "vitest";
import {
  PROPOSAL_KIND_HANDLERS,
  getProposalHandler,
  type AddTaskParsed,
  type AddEventParsed,
  type AddTransactionParsed,
  type SetBudgetParsed,
  type AddRecurringPaymentParsed,
  type AddContractParsed,
  type AddShoppingListItemParsed,
  type AddRecipeQuickParsed,
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
      "add_contract",
      "add_event",
      "add_maintenance_schedule",
      "add_recipe_quick",
      "add_recurring_block",
      "add_recurring_payment",
      "add_task",
      "add_time_block",
      "add_to_shopping_list",
      "add_transaction",
      "complete_chore",
      "complete_task",
      "set_budget",
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

  it("declares edit support", () => {
    expect(handler.supportsEdit).toBe(true);
    expect(handler.EditDrawer).toBeDefined();
  });
});

describe("add_transaction handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_transaction;

  it("parses transaction diffs", () => {
    const parsed = handler.parse(
      JSON.stringify({
        amount_pence: -1240,
        description: "Tesco Express",
        merchant: "Tesco",
        category_name: "Groceries",
      }),
    ) as AddTransactionParsed;
    expect(parsed.amount_pence).toBe(-1240);
    expect(parsed.category_name).toBe("Groceries");
  });

  it("summarises signed money and description", () => {
    expect(
      handler.summarise({
        amount_pence: -1240,
        currency: "GBP",
        description: "Tesco Express",
      }),
    ).toBe("Add transaction: -GBP 12.40 · Tesco Express");
  });
});

describe("set_budget handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.set_budget;

  it("parses budget diffs", () => {
    const parsed = handler.parse(
      JSON.stringify({ category_name: "Groceries", amount_pence: 40000 }),
    ) as SetBudgetParsed;
    expect(parsed.category_name).toBe("Groceries");
    expect(parsed.amount_pence).toBe(40000);
  });

  it("summarises category and amount", () => {
    expect(
      handler.summarise({ category_name: "Groceries", amount_pence: 40000 }),
    ).toBe("Set budget: Groceries · GBP 400.00");
  });
});

describe("add_recurring_payment handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_recurring_payment;

  it("parses recurring payment diffs", () => {
    const parsed = handler.parse(
      JSON.stringify({
        description: "Netflix",
        amount_pence: 1299,
        category_name: "Subscriptions",
        day_of_month: 15,
      }),
    ) as AddRecurringPaymentParsed;
    expect(parsed.description).toBe("Netflix");
    expect(parsed.day_of_month).toBe(15);
  });

  it("summarises amount and payment day", () => {
    expect(
      handler.summarise({
        description: "Netflix",
        amount_pence: 1299,
        currency: "GBP",
        day_of_month: 15,
      }),
    ).toBe("Add recurring payment: Netflix · GBP 12.99 on day 15");
  });
});

describe("add_contract handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_contract;

  it("parses contract diffs", () => {
    const parsed = handler.parse(
      JSON.stringify({
        provider: "Zen Internet",
        kind: "broadband",
        monthly_cost_pence: 3000,
        term_start: 1767225600,
        term_end: 1798761600,
      }),
    ) as AddContractParsed;
    expect(parsed.provider).toBe("Zen Internet");
    expect(parsed.kind).toBe("broadband");
  });

  it("summarises provider, amount, and renewal date", () => {
    expect(
      handler.summarise({
        provider: "Zen Internet",
        monthly_cost_pence: 3000,
        term_start: 1767225600,
        term_end: 1798761600,
      }),
    ).toMatch(/^Add contract: Zen Internet · GBP 30\.00\/mo · renews /);
  });
});

describe("add_to_shopping_list handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_to_shopping_list;

  it("parses a single shopping list item into a one-item array", () => {
    const parsed = handler.parse(
      JSON.stringify({ item: "milk" }),
    ) as AddShoppingListItemParsed[];
    expect(parsed).toHaveLength(1);
    expect(parsed[0].item).toBe("milk");
  });

  it("summarises bundles", () => {
    expect(handler.summarise([{ item: "milk" }, { item: "eggs" }])).toBe(
      "Add 2 shopping items",
    );
  });
});

describe("add_recipe_quick handler", () => {
  const handler = PROPOSAL_KIND_HANDLERS.add_recipe_quick;

  it("parses recipe diffs", () => {
    const parsed = handler.parse(
      JSON.stringify({
        title: "Miso pasta",
        ingredients: ["pasta", { quantity_text: "2 tbsp", ingredient_name: "miso" }],
        steps: ["Boil pasta", "Stir through miso"],
      }),
    ) as AddRecipeQuickParsed;
    expect(parsed.title).toBe("Miso pasta");
    expect(parsed.ingredients).toHaveLength(2);
  });

  it("summarises ingredient and step counts", () => {
    expect(
      handler.summarise({
        title: "Miso pasta",
        ingredients: ["pasta", "miso"],
        steps: ["Boil pasta", "Stir through miso"],
      }),
    ).toBe("Add recipe: Miso pasta · 2 ingredients · 2 steps");
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
