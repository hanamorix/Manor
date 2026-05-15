// Thin wrapper over ScheduleDrawer for proposal-edit mode.
//
// Phase 1.H. Lets the proposal-card registry mount the existing
// ScheduleDrawer for `add_maintenance_schedule` proposals without the
// registry having to know about MaintenanceSchedule shape internals.
//
// ScheduleDrawer manages its own approve flow via
// `pdf_extract_approve_with_override` — that's the path that preserves
// the AI-generated provenance fields (`source_attachment_uuid`, `tier`)
// the user's drawer doesn't surface. Generalising that logic into
// `approve_proposal_with_override` is sensible cleanup for v0.3, but
// keeping the existing wire shape here means Phase 1's "zero behaviour
// diff vs v0.1.5" gate holds.

import type { Proposal } from "../../lib/today/ipc";
import type { AddMaintenanceScheduleParsed } from "./registry";
import { ScheduleDrawer } from "../Bones/DueSoon/ScheduleDrawer";

export interface ProposalScheduleEditDrawerProps {
  parsed: AddMaintenanceScheduleParsed;
  proposal: Proposal;
  onClose: () => void;
  onApplied: () => void;
}

export function ProposalScheduleEditDrawer({
  parsed,
  proposal,
  onClose,
  onApplied,
}: ProposalScheduleEditDrawerProps) {
  const now = Math.floor(Date.now() / 1000);
  const syntheticSchedule = {
    id: "",
    asset_id: parsed.asset_id,
    task: parsed.task,
    interval_months: parsed.interval_months,
    last_done_date: parsed.last_done_date ?? null,
    next_due_date: "",
    notes: parsed.notes,
    created_at: now,
    updated_at: now,
    deleted_at: null,
  };

  return (
    <ScheduleDrawer
      schedule={syntheticSchedule}
      initialAssetId={parsed.asset_id}
      lockAsset={true}
      proposalId={proposal.id}
      onClose={onClose}
      onSaved={() => {
        onApplied();
        onClose();
      }}
    />
  );
}
