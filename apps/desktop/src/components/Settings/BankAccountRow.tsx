import { useState } from "react";
import type { BankAccount } from "../../lib/ledger/bank-ipc";
import { useBankStore } from "../../lib/ledger/bank-state";

interface Props {
  account: BankAccount;
  onReconnect: (account_id: number) => void;
}

export function BankAccountRow({ account, onReconnect }: Props) {
  const [busy, setBusy] = useState(false);
  const { syncNow, disconnect } = useBankStore();

  const now = Math.floor(Date.now() / 1000);
  const expired =
    account.sync_paused_reason === "requisition_expired" ||
    (account.requisition_expires_at !== null && account.requisition_expires_at < now);
  const daysLeft =
    account.requisition_expires_at !== null
      ? Math.max(0, Math.floor((account.requisition_expires_at - now) / 86400))
      : null;
  const lastSynced =
    account.last_synced_at !== null
      ? formatRelative(now - account.last_synced_at)
      : "never";

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "12px 16px",
        background: expired ? "#3a2a12" : "#1a1a2e",
        border: `1px solid ${expired ? "#b7791f" : "#2d2d4a"}`,
        borderRadius: 8,
        marginBottom: 8,
      }}
    >
      {account.institution_logo_url && (
        <img src={account.institution_logo_url} width={32} height={32} alt="" />
      )}
      <div style={{ flex: 1 }}>
        <div style={{ color: "#e4e4e7", fontWeight: 600 }}>
          {account.institution_name} · {account.account_name}
        </div>
        <div style={{ color: "#a1a1aa", fontSize: 12 }}>
          synced {lastSynced}
          {daysLeft !== null && ` · expires in ${daysLeft} days`}
        </div>
      </div>
      {expired ? (
        <button onClick={() => onReconnect(account.id)} disabled={busy}>
          Reconnect
        </button>
      ) : (
        <>
          <button
            onClick={async () => {
              setBusy(true);
              await syncNow(account.id);
              setBusy(false);
            }}
            disabled={busy}
          >
            ↻ Sync
          </button>
          <button
            onClick={async () => {
              if (!confirm(`Disconnect ${account.institution_name}?`)) return;
              setBusy(true);
              await disconnect(account.id);
              setBusy(false);
            }}
            disabled={busy}
          >
            ✕
          </button>
        </>
      )}
    </div>
  );
}

function formatRelative(seconds: number): string {
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}
