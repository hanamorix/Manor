// Registry of per-kind handlers for proposal cards.
//
// Phase 1.H of v0.2 Hands. Without this registry, every new tool kind
// spawns hardcoded `if (proposal.kind === ...)` branches across
// ProposalBanner.tsx, PendingProposalsBlock.tsx, and any future room.
// With 17 kinds in v0.2 that's a maintenance disaster — so we centralise
// here and Phase 1.J retires the hardcoded paths.
//
// New kinds (Phase 2+) add one entry to PROPOSAL_KIND_HANDLERS. No
// component code touches required.

import { createElement, type ComponentType } from "react";
import type { Proposal } from "../../lib/today/ipc";
import { ProposalEventEditDrawer } from "./ProposalEventEditDrawer";

/// Per-kind handler. Generic `P` is the parsed-diff shape.
///
/// - `parse` decodes the raw JSON into the typed shape. Throws on malformed
///   input — callers wrap in try/catch.
/// - `summarise` produces the single-line glance text shown in the card
///   header (e.g. "Add task: Buy milk", "Annual service · every 12 months").
/// - `CardBody` is an optional details slot. Most kinds don't need one in
///   Phase 1; subsequent phases will use it for richer previews.
/// - `supportsEdit` flags that the card should expose an Edit button.
/// - `EditDrawer` mounts when Edit is clicked. It is self-contained:
///   handles its own save/approve flow and calls `onApplied()` when done.
///   This matches the existing ScheduleDrawer shape (which already
///   manages its own provenance-preserving approve via
///   `pdf_extract_approve_with_override`).
export interface ProposalCardHandler<P> {
  parse: (diffJson: string) => P;
  summarise: (parsed: P) => string;
  CardBody?: ComponentType<{ parsed: P; proposal: Proposal }>;
  supportsEdit?: boolean;
  EditDrawer?: ComponentType<{
    parsed: P;
    proposal: Proposal;
    onClose: () => void;
    onApplied: () => void;
  }>;
}

// ── add_task ────────────────────────────────────────────────────────────

export interface AddTaskParsed {
  title: string;
  due_date?: string;
}

const addTaskHandler: ProposalCardHandler<AddTaskParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as AddTaskParsed,
  summarise: (parsed) => {
    const dateSuffix = parsed.due_date ? ` (due ${parsed.due_date})` : "";
    return `Add task: ${parsed.title}${dateSuffix}`;
  },
};

// ── complete_task ──────────────────────────────────────────────────────

export interface CompleteTaskParsed {
  task_id?: number;
  title?: string;
}

const completeTaskHandler: ProposalCardHandler<CompleteTaskParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as CompleteTaskParsed,
  summarise: (parsed) =>
    `Complete task: ${parsed.title ?? `#${parsed.task_id ?? "unknown"}`}`,
};

// ── add_event ──────────────────────────────────────────────────────────

export interface AddEventParsed {
  account_id?: number;
  calendar_url?: string;
  title: string;
  start_at: number;
  end_at: number;
  description?: string | null;
  location?: string | null;
  all_day?: boolean;
}

function normaliseEventDiff(
  parsed: AddEventParsed | AddEventParsed[],
): AddEventParsed[] {
  return Array.isArray(parsed) ? parsed : [parsed];
}

function formatEventTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

const addEventHandler: ProposalCardHandler<AddEventParsed[]> = {
  parse: (diffJson) =>
    normaliseEventDiff(JSON.parse(diffJson) as AddEventParsed | AddEventParsed[]),
  summarise: (parsed) => {
    if (parsed.length === 1) {
      const event = parsed[0];
      return `Add event: ${event?.title ?? "Untitled event"} (${event ? formatEventTime(event.start_at) : "time unknown"})`;
    }
    return `Add ${parsed.length} events`;
  },
  CardBody: ({ parsed }) => (
    createElement(
      "div",
      { style: { marginTop: 4, fontSize: 11, color: "var(--ink-soft)" } },
      `${parsed
        .slice(0, 3)
        .map((event) => `${event.title} · ${formatEventTime(event.start_at)}`)
        .join(" · ")}${parsed.length > 3 ? ` · +${parsed.length - 3} more` : ""}`,
    )
  ),
  supportsEdit: true,
  EditDrawer: ProposalEventEditDrawer,
};

// ── add_transaction ────────────────────────────────────────────────────

export interface AddTransactionParsed {
  amount_pence: number;
  currency?: string;
  description: string;
  merchant?: string | null;
  category_id?: number | null;
  category_name?: string | null;
  date?: number | null;
  note?: string | null;
}

function formatMoney(pence: number, currency = "GBP"): string {
  const sign = pence < 0 ? "-" : "";
  const amount = Math.abs(pence) / 100;
  return `${sign}${currency} ${amount.toFixed(2)}`;
}

const addTransactionHandler: ProposalCardHandler<AddTransactionParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as AddTransactionParsed,
  summarise: (parsed) =>
    `Add transaction: ${formatMoney(parsed.amount_pence, parsed.currency)} · ${parsed.description}`,
  CardBody: ({ parsed }) =>
    createElement(
      "div",
      { style: { marginTop: 4, fontSize: 11, color: "var(--ink-soft)" } },
      [
        parsed.merchant,
        parsed.category_name ?? (parsed.category_id ? `Category #${parsed.category_id}` : null),
      ]
        .filter(Boolean)
        .join(" · "),
    ),
};

// ── set_budget ─────────────────────────────────────────────────────────

export interface SetBudgetParsed {
  category_id?: number | null;
  category_name?: string | null;
  amount_pence: number;
}

const setBudgetHandler: ProposalCardHandler<SetBudgetParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as SetBudgetParsed,
  summarise: (parsed) =>
    `Set budget: ${parsed.category_name ?? `Category #${parsed.category_id ?? "unknown"}`} · ${formatMoney(parsed.amount_pence)}`,
};

// ── add_recurring_payment ──────────────────────────────────────────────

export interface AddRecurringPaymentParsed {
  description: string;
  amount_pence: number;
  currency?: string;
  category_id?: number | null;
  category_name?: string | null;
  day_of_month: number;
  note?: string | null;
}

const addRecurringPaymentHandler: ProposalCardHandler<AddRecurringPaymentParsed> =
  {
    parse: (diffJson) => JSON.parse(diffJson) as AddRecurringPaymentParsed,
    summarise: (parsed) =>
      `Add recurring payment: ${parsed.description} · ${formatMoney(parsed.amount_pence, parsed.currency)} on day ${parsed.day_of_month}`,
    CardBody: ({ parsed }) =>
      createElement(
        "div",
        { style: { marginTop: 4, fontSize: 11, color: "var(--ink-soft)" } },
        [
          parsed.category_name ?? (parsed.category_id ? `Category #${parsed.category_id}` : null),
          parsed.note,
        ]
          .filter(Boolean)
          .join(" · "),
      ),
  };

// ── add_contract ───────────────────────────────────────────────────────

export interface AddContractParsed {
  provider: string;
  kind?: string;
  description?: string | null;
  monthly_cost_pence: number;
  term_start: number;
  term_end: number;
  exit_fee_pence?: number | null;
  renewal_alert_days?: number;
  recurring_payment_id?: number | null;
  note?: string | null;
}

function formatDate(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

const addContractHandler: ProposalCardHandler<AddContractParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as AddContractParsed,
  summarise: (parsed) =>
    `Add contract: ${parsed.provider} · ${formatMoney(parsed.monthly_cost_pence)}/mo · renews ${formatDate(parsed.term_end)}`,
  CardBody: ({ parsed }) =>
    createElement(
      "div",
      { style: { marginTop: 4, fontSize: 11, color: "var(--ink-soft)" } },
      [
        parsed.kind,
        parsed.exit_fee_pence ? `Exit fee ${formatMoney(parsed.exit_fee_pence)}` : null,
        parsed.recurring_payment_id ? `Recurring #${parsed.recurring_payment_id}` : null,
      ]
        .filter(Boolean)
        .join(" · "),
    ),
};

// ── add_to_shopping_list ───────────────────────────────────────────────

export interface AddShoppingListItemParsed {
  item: string;
}

function normaliseShoppingListDiff(
  parsed: AddShoppingListItemParsed | AddShoppingListItemParsed[],
): AddShoppingListItemParsed[] {
  return Array.isArray(parsed) ? parsed : [parsed];
}

const addToShoppingListHandler: ProposalCardHandler<AddShoppingListItemParsed[]> =
  {
    parse: (diffJson) =>
      normaliseShoppingListDiff(
        JSON.parse(diffJson) as AddShoppingListItemParsed | AddShoppingListItemParsed[],
      ),
    summarise: (parsed) => {
      if (parsed.length === 1) {
        return `Add shopping item: ${parsed[0]?.item ?? "Untitled item"}`;
      }
      return `Add ${parsed.length} shopping items`;
    },
    CardBody: ({ parsed }) =>
      createElement(
        "div",
        { style: { marginTop: 4, fontSize: 11, color: "var(--ink-soft)" } },
        `${parsed
          .slice(0, 4)
          .map((item) => item.item)
          .join(" · ")}${parsed.length > 4 ? ` · +${parsed.length - 4} more` : ""}`,
      ),
  };

// ── add_recipe_quick ──────────────────────────────────────────────────

export interface AddRecipeQuickIngredientParsed {
  quantity_text?: string | null;
  ingredient_name: string;
  note?: string | null;
}

export interface AddRecipeQuickParsed {
  title: string;
  ingredients: Array<string | AddRecipeQuickIngredientParsed>;
  steps: string[];
  servings?: number | null;
  prep_time_mins?: number | null;
  cook_time_mins?: number | null;
}

function ingredientLabel(ingredient: string | AddRecipeQuickIngredientParsed): string {
  if (typeof ingredient === "string") {
    return ingredient;
  }
  return [ingredient.quantity_text, ingredient.ingredient_name]
    .filter(Boolean)
    .join(" ");
}

const addRecipeQuickHandler: ProposalCardHandler<AddRecipeQuickParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as AddRecipeQuickParsed,
  summarise: (parsed) =>
    `Add recipe: ${parsed.title} · ${parsed.ingredients.length} ingredients · ${parsed.steps.length} steps`,
  CardBody: ({ parsed }) =>
    createElement(
      "div",
      { style: { marginTop: 4, fontSize: 11, color: "var(--ink-soft)" } },
      `${parsed.ingredients
        .slice(0, 4)
        .map(ingredientLabel)
        .join(" · ")}${parsed.ingredients.length > 4 ? ` · +${parsed.ingredients.length - 4} more` : ""}`,
    ),
};

// ── add_chore ───────────────────────────────────────────────────────────

export interface AddChoreParsed {
  title: string;
  emoji?: string;
  rrule: string;
  first_due_ms?: number;
  rotation_names?: string[];
}

function normaliseChoreDiff(
  parsed: AddChoreParsed | AddChoreParsed[],
): AddChoreParsed[] {
  return Array.isArray(parsed) ? parsed : [parsed];
}

const addChoreHandler: ProposalCardHandler<AddChoreParsed[]> = {
  parse: (diffJson) =>
    normaliseChoreDiff(JSON.parse(diffJson) as AddChoreParsed | AddChoreParsed[]),
  summarise: (parsed) => {
    if (parsed.length === 1) {
      return `Add chore: ${parsed[0]?.title ?? "Untitled chore"}`;
    }
    return `Add ${parsed.length} chores`;
  },
  CardBody: ({ parsed }) => (
    createElement(
      "div",
      { style: { marginTop: 4, fontSize: 11, color: "var(--ink-soft)" } },
      `${parsed
        .slice(0, 3)
        .map((item) => item.title)
        .join(" · ")}${parsed.length > 3 ? ` · +${parsed.length - 3} more` : ""}`,
    )
  ),
};

// ── complete_chore ──────────────────────────────────────────────────────

export interface CompleteChoreParsed {
  chore_id?: number;
  title?: string;
  completed_by?: number;
  completed_by_name?: string;
}

const completeChoreHandler: ProposalCardHandler<CompleteChoreParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as CompleteChoreParsed,
  summarise: (parsed) =>
    `Complete chore: ${parsed.title ?? `#${parsed.chore_id ?? "unknown"}`}`,
};

// ── time blocks ────────────────────────────────────────────────────────

export interface AddTimeBlockParsed {
  title: string;
  kind?: string;
  date_ms: number;
  start_time: string;
  end_time: string;
}

export interface AddRecurringBlockParsed extends AddTimeBlockParsed {
  rrule: string;
}

function formatBlockTime(parsed: AddTimeBlockParsed): string {
  return `${parsed.start_time}-${parsed.end_time}`;
}

const addTimeBlockHandler: ProposalCardHandler<AddTimeBlockParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as AddTimeBlockParsed,
  summarise: (parsed) =>
    `Add block: ${parsed.title} (${formatBlockTime(parsed)})`,
};

const addRecurringBlockHandler: ProposalCardHandler<AddRecurringBlockParsed> = {
  parse: (diffJson) => JSON.parse(diffJson) as AddRecurringBlockParsed,
  summarise: (parsed) =>
    `Add recurring block: ${parsed.title} (${formatBlockTime(parsed)})`,
};

// ── add_maintenance_schedule ────────────────────────────────────────────

export interface AddMaintenanceScheduleParsed {
  asset_id: string;
  task: string;
  interval_months: number;
  notes: string;
  source_attachment_uuid: string;
  tier: string;
  last_done_date?: string | null;
}

import { ProposalScheduleEditDrawer } from "./ProposalScheduleEditDrawer";

const addMaintenanceScheduleHandler: ProposalCardHandler<AddMaintenanceScheduleParsed> =
  {
    parse: (diffJson) =>
      JSON.parse(diffJson) as AddMaintenanceScheduleParsed,
    summarise: (parsed) => {
      const unit = parsed.interval_months === 1 ? "month" : "months";
      return `${parsed.task} · every ${parsed.interval_months} ${unit}`;
    },
    supportsEdit: true,
    EditDrawer: ProposalScheduleEditDrawer,
  };

// ── registry ────────────────────────────────────────────────────────────

/// All proposal kinds known to the frontend. Phase 1 ships two; Phase 2+
/// adds one entry per new tool.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const PROPOSAL_KIND_HANDLERS: Record<string, ProposalCardHandler<any>> =
  {
    add_task: addTaskHandler,
    complete_task: completeTaskHandler,
    add_event: addEventHandler,
    add_transaction: addTransactionHandler,
    set_budget: setBudgetHandler,
    add_recurring_payment: addRecurringPaymentHandler,
    add_contract: addContractHandler,
    add_to_shopping_list: addToShoppingListHandler,
    add_recipe_quick: addRecipeQuickHandler,
    add_chore: addChoreHandler,
    complete_chore: completeChoreHandler,
    add_time_block: addTimeBlockHandler,
    add_recurring_block: addRecurringBlockHandler,
    add_maintenance_schedule: addMaintenanceScheduleHandler,
  };

/// Look up a handler by kind. Returns `null` for unknown kinds; callers
/// should render a fallback (`<ProposalCard>` falls back to the bare kind
/// string).
export function getProposalHandler(
  kind: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
): ProposalCardHandler<any> | null {
  return PROPOSAL_KIND_HANDLERS[kind] ?? null;
}
