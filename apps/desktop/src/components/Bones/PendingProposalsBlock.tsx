import { useEffect } from "react";
import { usePdfExtractStore } from "../../lib/pdf_extract/state";
import { useMaintenanceStore } from "../../lib/maintenance/state";
import { ProposalCard } from "../Proposal/ProposalCard";

interface Props {
  assetId: string;
}

export function PendingProposalsBlock({ assetId }: Props) {
  const { proposalsByAsset, loadForAsset } = usePdfExtractStore();
  const { loadForAsset: loadSchedules } = useMaintenanceStore();

  useEffect(() => {
    if (!proposalsByAsset[assetId]) void loadForAsset(assetId);
  }, [assetId, proposalsByAsset, loadForAsset]);

  const rows = proposalsByAsset[assetId] ?? [];
  if (rows.length === 0) return null;

  const refresh = () => {
    void loadForAsset(assetId);
    void loadSchedules(assetId);
  };

  return (
    <section style={{ marginTop: 24 }}>
      <h3 style={{ margin: "0 0 12px 0" }}>Proposed schedules</h3>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {rows.map((p) => (
          <ProposalCard
            key={p.id}
            proposal={p}
            onApplied={refresh}
            onRejected={refresh}
          />
        ))}
      </div>
    </section>
  );
}
