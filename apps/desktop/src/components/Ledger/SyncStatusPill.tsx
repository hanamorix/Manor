import { useEffect } from "react";
import { useBankStore } from "../../lib/ledger/bank-state";

export function SyncStatusPill() {
  const { accounts, syncStatus, refresh } = useBankStore();

  useEffect(() => {
    refresh();
  }, [refresh]);

  if (accounts.length === 0) return null;

  if (syncStatus.kind === "syncing") {
    return <Pill color="#3b82f6" text="⟳ syncing…" />;
  }

  const now = Math.floor(Date.now() / 1000);
  const expired = accounts.find(
    (a) =>
      a.sync_paused_reason === "requisition_expired" ||
      (a.requisition_expires_at !== null && a.requisition_expires_at < now),
  );
  if (expired) {
    return <Pill color="#b7791f" text={`⚠ reconnect ${expired.institution_name}`} />;
  }

  const mostRecent = accounts.reduce<number | null>((max, a) => {
    if (a.last_synced_at === null) return max;
    return max === null || a.last_synced_at > max ? a.last_synced_at : max;
  }, null);
  if (mostRecent === null) {
    return <Pill color="#71717a" text="not yet synced" />;
  }
  const diff = now - mostRecent;
  return <Pill color="#22c55e" text={`✓ synced ${formatRelative(diff)}`} />;
}

function Pill({ color, text }: { color: string; text: string }) {
  return (
    <span
      style={{
        display: "inline-block",
        padding: "4px 10px",
        background: `${color}22`,
        color,
        borderRadius: "var(--radius-md)",
        fontSize: "var(--text-xs)",
        border: `1px solid ${color}`,
      }}
    >
      {text}
    </span>
  );
}

function formatRelative(seconds: number): string {
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}
