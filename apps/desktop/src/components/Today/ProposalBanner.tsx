import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import {
  listProposals,
  listTasks,
  type Proposal,
} from "../../lib/today/ipc";
import { ProposalCard } from "../Proposal/ProposalCard";

export default function ProposalBanner() {
  const pending = useTodayStore((s) => s.pendingProposals);
  const setPendingProposals = useTodayStore((s) => s.setPendingProposals);
  const setTasks = useTodayStore((s) => s.setTasks);
  const removeProposal = useTodayStore((s) => s.removeProposal);

  useEffect(() => {
    void listProposals("pending").then(setPendingProposals);
  }, [setPendingProposals]);

  if (pending.length === 0) return null;

  const handleApplied = (p: Proposal) => async () => {
    removeProposal(p.id);
    try {
      const refreshed = await listTasks();
      setTasks(refreshed);
    } catch {
      void listProposals("pending").then(setPendingProposals);
    }
  };

  const handleRejected = (p: Proposal) => () => {
    removeProposal(p.id);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      {pending.map((p) => (
        <ProposalCard
          key={p.id}
          proposal={p}
          onApplied={() => void handleApplied(p)()}
          onRejected={handleRejected(p)}
        />
      ))}
    </div>
  );
}
