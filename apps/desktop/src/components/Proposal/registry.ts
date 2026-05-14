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
    add_chore: addChoreHandler,
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
