import { useEffect, useState } from "react";
import { useSettingsStore } from "../../lib/settings/state";
import { listCalendarAccounts } from "../../lib/settings/ipc";
import AccountRow from "./AccountRow";
import AddAccountForm from "./AddAccountForm";
import { BankAccountsSection } from "./BankAccountsSection";

export default function CalendarsTab() {
  const accounts = useSettingsStore((s) => s.accounts);
  const setAccounts = useSettingsStore((s) => s.setAccounts);
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    void listCalendarAccounts().then(setAccounts);
  }, [setAccounts]);

  return (
    <div style={{ padding: "14px 16px" }}>
      <p style={{
        fontSize: 11, textTransform: "uppercase", letterSpacing: 0.6,
        color: "rgba(0,0,0,0.55)", fontWeight: 700, margin: "0 0 10px",
      }}>Your calendar accounts</p>

      {accounts.length === 0 && !adding && (
        <p style={{ color: "rgba(0,0,0,0.5)", fontSize: 13, marginBottom: 12 }}>
          No accounts yet. Add one to start syncing events.
        </p>
      )}

      {accounts.map((a) => <AccountRow key={a.id} account={a} />)}

      {!adding && (
        <button
          onClick={() => setAdding(true)}
          style={{
            background: "transparent", border: "none",
            color: "var(--imessage-blue)", fontWeight: 700, fontSize: 12,
            padding: "6px 0", cursor: "pointer",
          }}
        >
          + Add calendar account
        </button>
      )}

      {adding && <AddAccountForm onClose={() => setAdding(false)} />}

      <BankAccountsSection />
    </div>
  );
}
