import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useWizardStore } from "../../lib/wizard/state";

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
        <h2 style={{ margin: "0 0 8px 0" }}>Your calendar</h2>
        <p style={{ fontSize: 13, color: "#aaa", lineHeight: 1.5 }}>
          Manor talks to your calendar over <strong>CalDAV</strong> — works with iCloud,
          Fastmail, Proton, Nextcloud, and most self-hosted servers. Google Calendar is
          not supported (by design: Google's servers in the loop contradicts local-first).
        </p>
      </div>

      {count === 0 && (
        <div
          style={{
            padding: 12,
            border: "1px solid #333",
            background: "#141414",
            borderRadius: 6,
          }}
        >
          <div style={{ fontSize: 13 }}>No calendar accounts connected yet.</div>
          <div style={{ fontSize: 12, color: "#888", marginTop: 6, lineHeight: 1.5 }}>
            Skip for now — you can add one any time from <strong>Settings → Calendars</strong>.
            Manor will work fine without it; you'll just see fewer events in the Today view.
          </div>
        </div>
      )}

      {count !== null && count > 0 && (
        <div
          style={{
            padding: 12,
            border: "1px solid #244",
            background: "#0f1f1f",
            borderRadius: 6,
          }}
        >
          <div style={{ color: "#6f6" }}>● {count} calendar account(s) already connected</div>
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button onClick={advance} style={{ padding: "8px 16px" }}>
          Next
        </button>
      </div>
    </div>
  );
}
