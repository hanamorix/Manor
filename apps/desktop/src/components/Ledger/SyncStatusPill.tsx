import { useEffect } from "react";
import type { LucideIcon } from "lucide-react";
import { RefreshCw, AlertTriangle, Check, Clock } from "lucide-react";
import { useBankStore } from "../../lib/ledger/bank-state";

export function SyncStatusPill() {
  const { accounts, syncStatus, refresh } = useBankStore();

  useEffect(() => {
    refresh();
  }, [refresh]);

  if (accounts.length === 0) return null;

  if (syncStatus.kind === "syncing") {
    return <Pill icon={RefreshCw} text="syncing…" variant="active" />;
  }

  const now = Math.floor(Date.now() / 1000);
  const expired = accounts.find(
    (a) =>
      a.sync_paused_reason === "requisition_expired" ||
      (a.requisition_expires_at !== null && a.requisition_expires_at < now),
  );
  if (expired) {
    return (
      <Pill
        icon={AlertTriangle}
        text={`reconnect ${expired.institution_name}`}
        variant="danger"
      />
    );
  }

  const mostRecent = accounts.reduce<number | null>((max, a) => {
    if (a.last_synced_at === null) return max;
    return max === null || a.last_synced_at > max ? a.last_synced_at : max;
  }, null);
  if (mostRecent === null) {
    return <Pill icon={Clock} text="not yet synced" variant="muted" />;
  }
  const diff = now - mostRecent;
  return <Pill icon={Check} text={`synced ${formatRelative(diff)}`} variant="ok" />;
}

type Variant = "active" | "danger" | "muted" | "ok";

function Pill({ icon: Icon, text, variant }: { icon: LucideIcon; text: string; variant: Variant }) {
  const color =
    variant === "danger" ? "var(--ink-danger)"
    : variant === "muted"  ? "var(--ink-faint)"
    :                        "var(--ink-soft)";
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
        padding: "4px 10px",
        background: "var(--paper-muted)",
        color,
        borderRadius: "var(--radius-md)",
        fontSize: "var(--text-xs)",
        border: "1px solid var(--hairline-strong)",
      }}
    >
      <Icon size={12} strokeWidth={1.8} />
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
