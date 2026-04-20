import type { AssetSpendTotal } from "../../../lib/maintenance/event-ipc";

interface Props {
  total: AssetSpendTotal;
  onOpen(): void;
  sortBy: "12m" | "lifetime";
}

const EMOJI: Record<string, string> = {
  appliance: "🏠",
  vehicle: "🚗",
  fixture: "🔧",
  other: "📦",
};

function gbp(pence: number): string {
  return `£${(pence / 100).toFixed(0)}`;
}

export function SpendAssetRow({ total, onOpen, sortBy }: Props) {
  const value =
    sortBy === "12m" ? total.total_last_12m_pence : total.total_lifetime_pence;
  return (
    <button
      type="button"
      onClick={onOpen}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        width: "100%",
        padding: "8px 0",
        background: "none",
        border: "none",
        borderBottom: "1px solid var(--border, #eee)",
        cursor: "pointer",
        textAlign: "left",
      }}
    >
      <span>{EMOJI[total.asset_category] ?? "📦"}</span>
      <span style={{ flex: 1 }}>{total.asset_name}</span>
      <span style={{ fontVariantNumeric: "tabular-nums" }}>{gbp(value)}</span>
    </button>
  );
}
