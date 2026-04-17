import { useEffect, useState } from "react";
import { Calendar, Plus } from "lucide-react";
import { useSettingsStore } from "../../lib/settings/state";
import { listCalendarAccounts } from "../../lib/settings/ipc";
import AccountRow from "./AccountRow";
import AddAccountForm from "./AddAccountForm";
import { BankAccountsSection } from "./BankAccountsSection";
import { SectionLabel, Button } from "../../lib/ui";

export default function CalendarsTab() {
  const accounts = useSettingsStore((s) => s.accounts);
  const setAccounts = useSettingsStore((s) => s.setAccounts);
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    void listCalendarAccounts().then(setAccounts);
  }, [setAccounts]);

  return (
    <div style={{ padding: "14px 16px" }}>
      <SectionLabel
        icon={Calendar}
        action={!adding ? <Button variant="secondary" icon={Plus} onClick={() => setAdding(true)}>Add account</Button> : undefined}
      >
        Your calendar accounts
      </SectionLabel>

      {accounts.length === 0 && !adding && (
        <p style={{ color: "var(--ink-soft)", fontSize: 13, marginBottom: 12 }}>
          No accounts yet. Add one to start syncing events.
        </p>
      )}

      {accounts.map((a) => <AccountRow key={a.id} account={a} />)}

      {adding && <AddAccountForm onClose={() => setAdding(false)} />}

      <BankAccountsSection />
    </div>
  );
}
