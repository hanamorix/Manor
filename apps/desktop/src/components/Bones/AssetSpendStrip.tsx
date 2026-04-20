import { useEffect, useState } from "react";
import * as eventIpc from "../../lib/maintenance/event-ipc";

interface Props {
  assetId: string;
}

function gbp(pence: number): string {
  return `£${(pence / 100).toFixed(0)}`;
}

export function AssetSpendStrip({ assetId }: Props) {
  const [total, setTotal] = useState<eventIpc.AssetSpendTotal | null>(null);

  useEffect(() => {
    let cancelled = false;
    eventIpc
      .spendForAsset(assetId)
      .then((t) => {
        if (!cancelled) setTotal(t);
      })
      .catch((e) => {
        console.error("AssetSpendStrip: spendForAsset failed", e);
      });
    return () => {
      cancelled = true;
    };
  }, [assetId]);

  if (!total) return null;
  if (total.total_lifetime_pence === 0 && total.event_count_lifetime === 0) {
    return null;
  }

  return (
    <div
      style={{
        display: "flex",
        gap: 16,
        color: "var(--ink-soft, #666)",
        fontSize: 13,
        margin: "8px 0 16px 0",
      }}
    >
      <span>
        12-month spend <strong>{gbp(total.total_last_12m_pence)}</strong>
      </span>
      <span>·</span>
      <span>
        Lifetime <strong>{gbp(total.total_lifetime_pence)}</strong>
      </span>
      <span>·</span>
      <span>
        {total.event_count_lifetime} completion
        {total.event_count_lifetime === 1 ? "" : "s"}
      </span>
    </div>
  );
}
