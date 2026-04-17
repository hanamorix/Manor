import { useEffect, useState } from "react";
import { useBankStore } from "../../lib/ledger/bank-state";
import { BankAccountRow } from "./BankAccountRow";
import { ConnectBankDrawer } from "./ConnectBankDrawer";

export function BankAccountsSection() {
  const { accounts, refresh } = useBankStore();
  const [drawerMode, setDrawerMode] = useState<
    { kind: "closed" } | { kind: "connect" } | { kind: "reconnect"; account_id: number }
  >({ kind: "closed" });

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <section style={{ marginTop: 24 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 12,
        }}
      >
        <h3 style={{ color: "var(--ink)", margin: 0 }}>Bank Accounts</h3>
        <button onClick={() => setDrawerMode({ kind: "connect" })}>+ Connect</button>
      </div>

      {accounts.length === 0 && (
        <div style={{ color: "var(--ink-soft)", padding: "16px 0" }}>
          No bank accounts connected yet.
        </div>
      )}

      {accounts.map((a) => (
        <BankAccountRow
          key={a.id}
          account={a}
          onReconnect={(id) => setDrawerMode({ kind: "reconnect", account_id: id })}
        />
      ))}

      {drawerMode.kind !== "closed" && (
        <ConnectBankDrawer
          mode={drawerMode}
          onClose={() => {
            setDrawerMode({ kind: "closed" });
            refresh();
          }}
        />
      )}
    </section>
  );
}
