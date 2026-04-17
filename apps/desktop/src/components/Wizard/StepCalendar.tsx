import { useEffect, useState } from "react";
import { ArrowRight } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useWizardStore } from "../../lib/wizard/state";
import { wizardStatusCardGood, wizardStatusCardMuted } from "./styles";
import { Button } from "../../lib/ui";

interface CalendarAccount {
  id: number;
  email: string;
}

export default function StepCalendar() {
  const advance = useWizardStore((s) => s.advance);
  const [count, setCount] = useState<number | null>(null);

  useEffect(() => {
    void invoke<CalendarAccount[]>("list_calendar_accounts")
      .then((accounts) => setCount(accounts.length))
      .catch(() => setCount(0));
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 16, color: "var(--ink)" }}>
          Your calendar
        </h2>
        <p style={{ fontSize: 13, color: "var(--ink-soft)", lineHeight: 1.5, margin: 0 }}>
          Manor talks to your calendar over <strong>CalDAV</strong> — works with iCloud,
          Fastmail, Proton, Nextcloud, and most self-hosted servers. Google Calendar is
          not supported (by design: Google's servers in the loop contradicts local-first).
        </p>
      </div>

      {count === 0 && (
        <div style={wizardStatusCardMuted}>
          <div style={{ fontSize: 13, color: "var(--ink)", fontWeight: 500 }}>
            No calendar accounts connected yet.
          </div>
          <div
            style={{
              fontSize: 12,
              color: "var(--ink-soft)",
              marginTop: 6,
              lineHeight: 1.5,
            }}
          >
            Skip for now — you can add one any time from{" "}
            <strong>Settings → Calendars</strong>. Manor will work fine without it; you'll
            just see fewer events in the Today view.
          </div>
        </div>
      )}

      {count !== null && count > 0 && (
        <div style={wizardStatusCardGood}>
          <div style={{ color: "var(--ink)", fontWeight: 600, fontSize: 13 }}>
            ● {count} calendar account(s) already connected
          </div>
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <Button variant="primary" icon={ArrowRight} onClick={advance}>
          Next
        </Button>
      </div>
    </div>
  );
}
