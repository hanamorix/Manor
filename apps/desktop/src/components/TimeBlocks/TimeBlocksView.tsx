import { useEffect, useState } from "react";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { listBlocksForWeek, listRecurringBlocks, type TimeBlock, type BlockKind } from "../../lib/timeblocks/ipc";
import BlockDrawer from "./BlockDrawer";

const pageStyle: React.CSSProperties = {
  maxWidth: 760,
  margin: "0 auto",
  padding: "24px 24px 120px",
};

const sectionStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
  marginBottom: 12,
};

const headerStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "var(--ink-soft)",
  marginBottom: 10,
};

const dayHeading: React.CSSProperties = {
  fontSize: 12,
  fontWeight: 700,
  color: "rgba(20,20,30,0.6)",
  marginTop: 12,
  marginBottom: 4,
};

const KIND_COLOR: Record<BlockKind, string> = {
  focus: "#007aff",
  errands: "#FFC15C",
  admin: "#9b59b6",
  dnd: "#ff3b30",
};

const KIND_LABEL: Record<BlockKind, string> = {
  focus: "Focus",
  errands: "Errands",
  admin: "Admin",
  dnd: "DND",
};

const rowStyle = (kind: BlockKind): React.CSSProperties => ({
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 10px",
  borderLeft: `3px solid ${KIND_COLOR[kind]}`,
  background: "rgba(20,20,30,0.02)",
  borderRadius: 6,
  marginBottom: 4,
  cursor: "pointer",
  fontSize: 13,
});

const addBtn: React.CSSProperties = {
  background: "var(--ink)",
  color: "var(--action-fg)",
  border: "none",
  borderRadius: 999,
  padding: "10px 20px",
  fontSize: 14,
  fontWeight: 600,
  cursor: "pointer",
  marginTop: 12,
};

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

function weekStartMs(): number {
  const now = new Date();
  const utcDay = (now.getUTCDay() + 6) % 7; // 0 = Monday
  const utcMidnight = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
  return utcMidnight - utcDay * 86_400_000;
}

function rruleToEnglish(rrule: string): string {
  if (rrule.includes("FREQ=WEEKLY") && rrule.includes("BYDAY=")) {
    const day = rrule.match(/BYDAY=([A-Z]{2})/)?.[1];
    const map: Record<string, string> = {
      MO: "Mondays", TU: "Tuesdays", WE: "Wednesdays", TH: "Thursdays",
      FR: "Fridays", SA: "Saturdays", SU: "Sundays",
    };
    return `Every ${map[day ?? ""] ?? "week"}`;
  }
  if (rrule.includes("FREQ=WEEKDAY") || rrule === "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR") {
    return "Every weekday";
  }
  if (rrule === "FREQ=DAILY") return "Every day";
  if (rrule === "FREQ=MONTHLY") return "Every month";
  return rrule;
}

export default function TimeBlocksView() {
  const weekBlocks = useTimeBlocksStore((s) => s.weekBlocks);
  const setWeekBlocks = useTimeBlocksStore((s) => s.setWeekBlocks);
  const recurring = useTimeBlocksStore((s) => s.recurringBlocks);
  const setRecurring = useTimeBlocksStore((s) => s.setRecurringBlocks);

  const [editing, setEditing] = useState<TimeBlock | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    void listBlocksForWeek(weekStartMs()).then(setWeekBlocks);
    void listRecurringBlocks().then(setRecurring);
  }, [setWeekBlocks, setRecurring]);

  // Group week blocks by weekday
  const byDay: Record<string, TimeBlock[]> = Object.fromEntries(DAYS.map((d) => [d, []]));
  for (const b of weekBlocks) {
    const wd = new Date(b.date).getUTCDay();
    const name = DAYS[(wd + 6) % 7];
    byDay[name].push(b);
  }

  return (
    <div style={pageStyle}>
      <h1 style={{ fontSize: 24, fontWeight: 700, margin: "0 0 16px" }}>Time Blocks</h1>

      <section style={sectionStyle}>
        <h2 style={headerStyle}>This week</h2>
        {weekBlocks.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>No blocks this week yet.</p>
        ) : (
          DAYS.map((day) => {
            const bs = byDay[day];
            if (bs.length === 0) return null;
            return (
              <div key={day}>
                <div style={dayHeading}>{day}</div>
                {[...bs].sort((a, b) => a.start_time.localeCompare(b.start_time)).map((b) => (
                  <div key={b.id} style={rowStyle(b.kind)} onClick={() => setEditing(b)}>
                    <strong style={{ color: KIND_COLOR[b.kind], fontWeight: 700, fontSize: 10, textTransform: "uppercase", letterSpacing: 0.5, minWidth: 50 }}>
                      {KIND_LABEL[b.kind]}
                    </strong>
                    <span style={{ flex: 1 }}>{b.title}</span>
                    <span style={{ color: "rgba(20,20,30,0.5)", fontSize: 12 }}>
                      {b.start_time}–{b.end_time}
                    </span>
                  </div>
                ))}
              </div>
            );
          })
        )}
      </section>

      <section style={sectionStyle}>
        <h2 style={headerStyle}>Recurring patterns</h2>
        {recurring.length === 0 ? (
          <p style={{ color: "rgba(20,20,30,0.5)", fontSize: 13, margin: 0 }}>No patterns yet. Nell will suggest one when she notices a repetition.</p>
        ) : (
          recurring.map((b) => (
            <div key={b.id} style={rowStyle(b.kind)} onClick={() => setEditing(b)}>
              <strong style={{ color: KIND_COLOR[b.kind], fontWeight: 700, fontSize: 10, textTransform: "uppercase", letterSpacing: 0.5, minWidth: 50 }}>
                {KIND_LABEL[b.kind]}
              </strong>
              <span style={{ flex: 1 }}>{b.title}</span>
              <span style={{ color: "rgba(20,20,30,0.5)", fontSize: 12 }}>
                {b.rrule ? rruleToEnglish(b.rrule) : ""} · {b.start_time}–{b.end_time}
              </span>
            </div>
          ))
        )}
      </section>

      <button style={addBtn} onClick={() => setCreating(true)}>+ Add block</button>

      {creating && <BlockDrawer block={null} onClose={() => setCreating(false)} />}
      {editing && <BlockDrawer block={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}
