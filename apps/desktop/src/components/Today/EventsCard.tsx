import { useState } from "react";
import { Calendar } from "lucide-react";
import { useTodayStore } from "../../lib/today/state";
import { useSettingsStore } from "../../lib/settings/state";
import { listEventsToday } from "../../lib/today/ipc";
import type { Event } from "../../lib/today/ipc";
import { SectionLabel } from "../../lib/ui";
import AddEventDrawer from "./AddEventDrawer";
import EditEventDrawer from "./EditEventDrawer";

function formatTime(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export default function EventsCard() {
  const events = useTodayStore((s) => s.events);
  const setEvents = useTodayStore((s) => s.setEvents);
  const accounts = useSettingsStore((s) => s.accounts);

  const [showAdd, setShowAdd] = useState(false);
  const [editingEvent, setEditingEvent] = useState<Event | null>(null);

  const firstAccount = accounts[0] ?? null;
  const canAdd = firstAccount !== null && firstAccount.default_calendar_url !== null;

  async function reloadEvents() {
    const fresh = await listEventsToday();
    setEvents(fresh);
  }

  return (
    <section style={{ marginBottom: 22 }}>
      <SectionLabel
        icon={Calendar}
        action={canAdd ? (
          <button
            onClick={() => setShowAdd(true)}
            style={{
              background: "none",
              border: "none",
              fontSize: 18,
              lineHeight: 1,
              cursor: "pointer",
              color: "var(--ink)",
              padding: "0 2px",
              fontWeight: 300,
            }}
            title="Add event"
          >
            +
          </button>
        ) : undefined}
      >
        Events
      </SectionLabel>

      {events.length === 0 ? (
        <p style={{ fontStyle: "italic", color: "var(--ink-faint)", margin: 0, fontSize: 13 }}>
          No events today.
        </p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          {events.map((e) => (
            <div
              key={e.id}
              onClick={() => setEditingEvent(e)}
              style={{ display: "flex", gap: 10, padding: "4px 0", fontSize: 13, cursor: "pointer" }}
            >
              <span style={{ fontWeight: 700, minWidth: 48, color: "var(--ink)", fontFamily: "var(--font-mono)" }}>
                {formatTime(e.start_at)}
              </span>
              <span>{e.title}</span>
            </div>
          ))}
        </div>
      )}

      {showAdd && firstAccount && firstAccount.default_calendar_url && (
        <AddEventDrawer
          accountId={firstAccount.id}
          defaultCalendarUrl={firstAccount.default_calendar_url}
          calendars={[]}
          onClose={() => setShowAdd(false)}
          onSaved={async () => {
            setShowAdd(false);
            await reloadEvents();
          }}
        />
      )}

      {editingEvent && (
        <EditEventDrawer
          event={editingEvent}
          onClose={() => setEditingEvent(null)}
          onSaved={async () => {
            setEditingEvent(null);
            await reloadEvents();
          }}
        />
      )}
    </section>
  );
}
