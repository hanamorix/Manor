import { useEffect, useState } from "react";
import { useSpendStore } from "../../../lib/maintenance/spend-state";
import { useBonesViewStore } from "../../../lib/bones/view-state";
import { SpendCategoryStrip } from "./SpendCategoryStrip";
import { SpendAssetRow } from "./SpendAssetRow";

export function SpendView() {
  const { assetTotals, categoryTotals, refresh, loadStatus } = useSpendStore();
  const { openAssetDetail } = useBonesViewStore();
  const [sortBy, setSortBy] = useState<"12m" | "lifetime">("12m");

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const sorted = [...assetTotals].sort((a, b) => {
    const av = sortBy === "12m" ? a.total_last_12m_pence : a.total_lifetime_pence;
    const bv = sortBy === "12m" ? b.total_last_12m_pence : b.total_lifetime_pence;
    return bv - av;
  });

  const noEvents = assetTotals.every((t) => t.event_count_lifetime === 0);

  if (loadStatus.kind === "error") {
    return (
      <div>
        <p>Couldn't load spend totals.</p>
        <button type="button" onClick={() => void refresh()}>
          Retry
        </button>
      </div>
    );
  }

  if (noEvents && assetTotals.length > 0) {
    return (
      <div
        style={{
          color: "var(--ink-soft, #888)",
          textAlign: "center",
          padding: 48,
        }}
      >
        No maintenance spend logged yet. Mark things done to start tracking.
      </div>
    );
  }

  return (
    <div>
      <h2 style={{ marginTop: 0 }}>12-month spend across the house</h2>
      <SpendCategoryStrip totals={categoryTotals} />

      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginBottom: 12,
        }}
      >
        <h3 style={{ margin: 0, flex: 1 }}>Per asset</h3>
        <select
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value as "12m" | "lifetime")}
        >
          <option value="12m">Sort by 12m spend</option>
          <option value="lifetime">Sort by lifetime spend</option>
        </select>
      </div>

      {sorted.map((t) => (
        <SpendAssetRow
          key={t.asset_id}
          total={t}
          sortBy={sortBy}
          onOpen={() => openAssetDetail(t.asset_id)}
        />
      ))}
    </div>
  );
}
