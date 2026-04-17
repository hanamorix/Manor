import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import {
  approveProposal,
  rejectProposal,
  listProposals,
  type Proposal,
} from "../../lib/today/ipc";

interface DiffSummary {
  title: string;
  due_date?: string;
}

function summarise(proposal: Proposal): string {
  if (proposal.kind === "add_task") {
    try {
      const parsed = JSON.parse(proposal.diff) as DiffSummary;
      const dateSuffix = parsed.due_date ? ` (due ${parsed.due_date})` : "";
      return `Add task: ${parsed.title}${dateSuffix}`;
    } catch {
      return "Add task";
    }
  }
  return proposal.kind;
}

function rationaleLine(proposal: Proposal): string {
  const r = proposal.rationale.trim();
  if (r.length === 0) return "Manor proposed this from your message";
  const truncated = r.length > 120 ? `${r.slice(0, 117)}...` : r;
  return `Manor: "${truncated}"`;
}

export default function ProposalBanner() {
  const pending = useTodayStore((s) => s.pendingProposals);
  const setPendingProposals = useTodayStore((s) => s.setPendingProposals);
  const setTasks = useTodayStore((s) => s.setTasks);
  const removeProposal = useTodayStore((s) => s.removeProposal);

  useEffect(() => {
    void listProposals("pending").then(setPendingProposals);
  }, [setPendingProposals]);

  if (pending.length === 0) return null;

  const handleApprove = async (id: number) => {
    removeProposal(id);
    try {
      const refreshedTasks = await approveProposal(id);
      setTasks(refreshedTasks);
    } catch {
      void listProposals("pending").then(setPendingProposals);
    }
  };

  const handleReject = async (id: number) => {
    removeProposal(id);
    try {
      await rejectProposal(id);
    } catch {
      void listProposals("pending").then(setPendingProposals);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      {pending.map((p) => (
        <div
          key={p.id}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            background: "var(--hairline)",
            border: "1px solid var(--hairline-strong)",
            borderRadius: "var(--radius-md)",
            padding: "10px 14px",
            animation: "bannerIn 200ms ease-out",
          }}
        >
          <span
            style={{
              background: "var(--ink)",
              color: "var(--action-fg)",
              fontSize: 10,
              fontWeight: 600,
              padding: "2px 8px",
              borderRadius: "var(--radius-md)",
              letterSpacing: 0.6,
              flexShrink: 0,
            }}
          >
            PROPOSAL
          </span>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontWeight: 600, fontSize: 13, color: "var(--ink)" }}>
              {summarise(p)}
            </div>
            <div
              style={{
                fontSize: 11,
                color: "var(--ink-soft)",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {rationaleLine(p)}
            </div>
          </div>
          <button
            onClick={() => void handleApprove(p.id)}
            aria-label="approve"
            style={{
              padding: "4px 10px",
              borderRadius: "var(--radius-md)",
              fontSize: 12,
              fontWeight: 600,
              border: "none",
              background: "var(--ink)",
              color: "var(--action-fg)",
              cursor: "pointer",
            }}
          >
            ✓
          </button>
          <button
            onClick={() => void handleReject(p.id)}
            aria-label="reject"
            style={{
              padding: "4px 10px",
              borderRadius: "var(--radius-md)",
              fontSize: 12,
              fontWeight: 600,
              border: "none",
              background: "var(--surface)",
              color: "var(--ink-soft)",
              cursor: "pointer",
            }}
          >
            ✗
          </button>
        </div>
      ))}
    </div>
  );
}
