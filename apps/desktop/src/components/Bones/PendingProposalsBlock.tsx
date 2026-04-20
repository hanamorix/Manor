import { useEffect, useState } from "react";
import { Check, Pencil, X } from "lucide-react";
import { usePdfExtractStore } from "../../lib/pdf_extract/state";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import type { Proposal } from "../../lib/today/ipc";
import { ScheduleDrawer } from "./DueSoon/ScheduleDrawer";

interface Props {
  assetId: string;
}

interface ParsedDiff {
  asset_id: string;
  task: string;
  interval_months: number;
  notes: string;
  source_attachment_uuid: string;
  tier: string;
}

function parseDiff(proposal: Proposal): ParsedDiff | null {
  try {
    return JSON.parse(proposal.diff) as ParsedDiff;
  } catch {
    return null;
  }
}

const buttonStyle: React.CSSProperties = {
  background: "transparent",
  border: "1px solid var(--border, #ddd)",
  borderRadius: 4,
  padding: 6,
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

export function PendingProposalsBlock({ assetId }: Props) {
  const { proposalsByAsset, loadForAsset, approveAsIs, reject } =
    usePdfExtractStore();
  const { loadForAsset: loadSchedules } = useMaintenanceStore();

  const [editProposal, setEditProposal] = useState<Proposal | null>(null);

  useEffect(() => {
    if (!proposalsByAsset[assetId]) void loadForAsset(assetId);
  }, [assetId, proposalsByAsset, loadForAsset]);

  const rows = proposalsByAsset[assetId] ?? [];

  if (rows.length === 0) return null;

  const onApprove = async (id: number) => {
    try {
      await approveAsIs(id, assetId);
      await loadSchedules(assetId);
    } catch (e: unknown) {
      console.error("approve failed", e);
    }
  };

  const onReject = async (id: number) => {
    try {
      await reject(id, assetId);
    } catch (e: unknown) {
      console.error("reject failed", e);
    }
  };

  return (
    <section style={{ marginTop: 24 }}>
      <h3 style={{ margin: "0 0 12px 0" }}>Proposed schedules</h3>
      <div
        style={{
          border: "1px solid var(--border, #e5e5e5)",
          borderRadius: 6,
          padding: 12,
          background: "var(--surface-subtle, #fafafa)",
        }}
      >
        {rows.map((p) => {
          const diff = parseDiff(p);
          if (!diff) return null;
          return (
            <div
              key={p.id}
              style={{
                display: "flex",
                alignItems: "flex-start",
                gap: 8,
                padding: "8px 0",
                borderBottom: "1px solid var(--border, #eee)",
              }}
            >
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 500 }}>
                  {diff.task} · every {diff.interval_months} month
                  {diff.interval_months === 1 ? "" : "s"}
                </div>
                {p.rationale.trim() && (
                  <div
                    style={{
                      fontSize: 12,
                      color: "var(--ink-soft, #888)",
                      fontStyle: "italic",
                      marginTop: 2,
                      display: "-webkit-box",
                      WebkitLineClamp: 2,
                      WebkitBoxOrient: "vertical",
                      overflow: "hidden",
                    }}
                  >
                    &ldquo;{p.rationale}&rdquo;
                  </div>
                )}
              </div>
              <button
                type="button"
                onClick={() => void onApprove(p.id)}
                aria-label="Approve proposal"
                title="Approve as-is"
                style={buttonStyle}
              >
                <Check size={16} />
              </button>
              <button
                type="button"
                onClick={() => setEditProposal(p)}
                aria-label="Edit proposal"
                title="Edit then approve"
                style={buttonStyle}
              >
                <Pencil size={14} />
              </button>
              <button
                type="button"
                onClick={() => void onReject(p.id)}
                aria-label="Reject proposal"
                title="Reject"
                style={buttonStyle}
              >
                <X size={14} />
              </button>
            </div>
          );
        })}
      </div>

      {editProposal &&
        (() => {
          const diff = parseDiff(editProposal);
          if (!diff) return null;
          const now = Math.floor(Date.now() / 1000);
          const syntheticSchedule = {
            id: "",
            asset_id: diff.asset_id,
            task: diff.task,
            interval_months: diff.interval_months,
            last_done_date: null,
            next_due_date: "",
            notes: diff.notes,
            created_at: now,
            updated_at: now,
            deleted_at: null,
          };
          return (
            <ScheduleDrawer
              schedule={syntheticSchedule}
              initialAssetId={diff.asset_id}
              lockAsset={true}
              proposalId={editProposal.id}
              onClose={() => setEditProposal(null)}
              onSaved={() => {
                setEditProposal(null);
                void loadForAsset(assetId);
                void loadSchedules(assetId);
              }}
            />
          );
        })()}
    </section>
  );
}
