import { useTodayStore } from "../../lib/today/state";

const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
};

const sectionHeader: React.CSSProperties = {
  fontSize: 11, textTransform: "uppercase", letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)", fontWeight: 700,
  margin: 0, marginBottom: 8,
};

function formatTime(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export default function EventsCard() {
  const events = useTodayStore((s) => s.events);

  return (
    <div style={cardStyle}>
      <p style={sectionHeader}>Events</p>
      {events.length === 0 ? (
        <p style={{ fontStyle: "italic", color: "rgba(0,0,0,0.5)", margin: 0, fontSize: 13 }}>
          No events today.
        </p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          {events.map((e) => (
            <div key={e.id} style={{ display: "flex", gap: 10, padding: "4px 0", fontSize: 13 }}>
              <span style={{ fontWeight: 700, minWidth: 48, color: "var(--imessage-blue)" }}>
                {formatTime(e.start_at)}
              </span>
              <span>{e.title}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
