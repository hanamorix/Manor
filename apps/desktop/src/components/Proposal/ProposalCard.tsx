// Generic proposal card. Reads from PROPOSAL_KIND_HANDLERS and renders
// approve/reject/edit affordances driven by the registry.
//
// Phase 1.I of v0.2 Hands. ProposalBanner and PendingProposalsBlock both
// retire their hardcoded `if (kind === ...)` blocks and render this
// component per pending proposal.
//
// Visual chrome ports the v0.1.5 banner row verbatim — bordered hairline
// frame, PROPOSAL pill, summary + rationale, action buttons. The
// document-flat .impeccable.md aesthetic is preserved.
//
// Inline error rendering pattern-matches on the typed ApplyError union
// (the wire shape from Phase 1.F): StaleReference / InvalidArg /
// Conflict / Network / UnknownKind / Internal each get a human-friendly
// rendering tuned to that variant.

import { useState } from "react";
import { Check, Pencil, X } from "lucide-react";
import {
  approveProposal,
  rejectProposal,
  isApplyError,
  type Applied,
  type ApplyError,
  type Proposal,
} from "../../lib/today/ipc";
import { getProposalHandler } from "./registry";

export interface ProposalCardProps {
  proposal: Proposal;
  onApplied?: (applied: Applied) => void;
  onRejected?: () => void;
  onError?: (err: ApplyError | string) => void;
}

export function ProposalCard({
  proposal,
  onApplied,
  onRejected,
  onError,
}: ProposalCardProps) {
  const handler = getProposalHandler(proposal.kind);
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [inlineError, setInlineError] = useState<string | null>(null);

  if (!handler) {
    return (
      <div style={cardOuterStyle}>
        <span style={pillStyle}>PROPOSAL</span>
        <div style={{ flex: 1, fontSize: "var(--text-sm)", color: "var(--ink-soft)" }}>
          Unsupported kind: {proposal.kind}
        </div>
      </div>
    );
  }

  let parsed: unknown;
  try {
    parsed = handler.parse(proposal.diff);
  } catch {
    return (
      <div style={cardOuterStyle}>
        <span style={pillStyle}>PROPOSAL</span>
        <div style={{ flex: 1, fontSize: "var(--text-sm)", color: "var(--ink-soft)" }}>
          {proposal.kind} (could not parse details)
        </div>
      </div>
    );
  }

  const summary = handler.summarise(parsed);
  const rationaleText = renderRationale(proposal);
  const CardBody = handler.CardBody;
  const EditDrawer = handler.EditDrawer;

  const handleApprove = async () => {
    setBusy(true);
    setInlineError(null);
    try {
      const applied = await approveProposal(proposal.id);
      onApplied?.(applied);
    } catch (e: unknown) {
      const err = isApplyError(e) ? e : null;
      setInlineError(err ? renderApplyError(err) : "Couldn't approve. Try again.");
      onError?.(err ?? String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleReject = async () => {
    setBusy(true);
    setInlineError(null);
    try {
      await rejectProposal(proposal.id);
      onRejected?.();
    } catch (e: unknown) {
      setInlineError("Couldn't reject. Try again.");
      onError?.(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div style={cardOuterStyle}>
        <span style={pillStyle}>PROPOSAL</span>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={summaryStyle}>{summary}</div>
          {rationaleText && <div style={rationaleStyle}>{rationaleText}</div>}
          {CardBody && <CardBody parsed={parsed} proposal={proposal} />}
          {inlineError && (
            <div style={errorStyle} role="alert">
              {inlineError}
            </div>
          )}
        </div>
        <button
          type="button"
          onClick={() => void handleApprove()}
          aria-label="approve"
          disabled={busy}
          style={approveButtonStyle}
        >
          <Check size={12} strokeWidth={2.2} />
        </button>
        {handler.supportsEdit && EditDrawer && (
          <button
            type="button"
            onClick={() => setEditing(true)}
            aria-label="edit"
            disabled={busy}
            style={ghostButtonStyle}
          >
            <Pencil size={12} strokeWidth={2.2} />
          </button>
        )}
        <button
          type="button"
          onClick={() => void handleReject()}
          aria-label="reject"
          disabled={busy}
          style={ghostButtonStyle}
        >
          <X size={12} strokeWidth={2.2} />
        </button>
      </div>
      {editing && EditDrawer && (
        <EditDrawer
          parsed={parsed}
          proposal={proposal}
          onClose={() => setEditing(false)}
          onApplied={() => {
            // Drawer self-manages approval — surface as if it had returned
            // an Applied. We don't have the Applied object here, so
            // synthesise one for the callback contract.
            onApplied?.({
              proposal_id: proposal.id,
              status: "applied",
              items_applied: 1,
              items_failed: 0,
              errors: [],
            });
          }}
        />
      )}
    </>
  );
}

// ── error rendering ─────────────────────────────────────────────────────

function renderApplyError(err: ApplyError): string {
  switch (err.type) {
    case "StaleReference":
      return `The ${err.value.entity} is no longer there. Try again.`;
    case "InvalidArg":
      return `That ${err.value.field} won't work — ${err.value.reason}.`;
    case "Conflict":
      return `Already handled. ${err.value}`;
    case "Network":
      return `Couldn't reach the calendar. ${err.value}`;
    case "UnknownKind":
      return `Manor doesn't know how to apply this yet (${err.value}).`;
    case "Internal":
      return `Something went wrong. ${err.value}`;
  }
}

function renderRationale(p: Proposal): string {
  const r = p.rationale.trim();
  if (r.length === 0) return "Manor proposed this from your message";
  const truncated = r.length > 120 ? `${r.slice(0, 117)}...` : r;
  return `Manor: "${truncated}"`;
}

// ── styles ──────────────────────────────────────────────────────────────

const cardOuterStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  background: "var(--hairline)",
  border: "1px solid var(--hairline-strong)",
  borderRadius: "var(--radius-md)",
  padding: "10px 14px",
  animation: "bannerIn 200ms ease-out",
};

const pillStyle: React.CSSProperties = {
  background: "var(--ink)",
  color: "var(--action-fg)",
  fontSize: 10,
  fontWeight: 600,
  padding: "2px 8px",
  borderRadius: "var(--radius-md)",
  flexShrink: 0,
};

const summaryStyle: React.CSSProperties = {
  fontWeight: 600,
  fontSize: "var(--text-sm)",
  color: "var(--ink)",
};

const rationaleStyle: React.CSSProperties = {
  fontSize: 11,
  color: "var(--ink-soft)",
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
};

const errorStyle: React.CSSProperties = {
  fontSize: 11,
  color: "var(--ink)",
  marginTop: 4,
  fontStyle: "italic",
};

const approveButtonStyle: React.CSSProperties = {
  padding: "4px 10px",
  borderRadius: "var(--radius-md)",
  fontSize: "var(--text-xs)",
  fontWeight: 600,
  border: "none",
  background: "var(--ink)",
  color: "var(--action-fg)",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
};

const ghostButtonStyle: React.CSSProperties = {
  padding: "4px 10px",
  borderRadius: "var(--radius-md)",
  fontSize: "var(--text-xs)",
  fontWeight: 600,
  border: "none",
  background: "var(--surface)",
  color: "var(--ink-soft)",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
};
