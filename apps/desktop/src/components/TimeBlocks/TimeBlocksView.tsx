import { useEffect, useState } from "react";
import { LayoutGrid, RefreshCw, Target, ShoppingCart, Inbox, BellOff, Plus } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useTimeBlocksStore } from "../../lib/timeblocks/state";
import { listBlocksForWeek, listRecurringBlocks, type TimeBlock, type BlockKind } from "../../lib/timeblocks/ipc";
import { PageHeader, SectionLabel, Button } from "../../lib/ui";
import BlockDrawer from "./BlockDrawer";

const pageStyle: React.CSSProperties = {
  maxWidth: 760,
  margin: "0 auto",
  padding: "24px 24px 120px",
};

const sectionStyle: React.CSSProperties = {
  marginBottom: 22,
};

const dayHeading: React.CSSProperties = {
  fontSize: "var(--text-xs)",
  fontWeight: 500,
  color: "var(--ink-soft)",
  marginTop: 12,
  marginBottom: 4,
};

const KIND_ICON: Record<BlockKind, LucideIcon> = {
  focus: Target,
  errands: ShoppingCart,
  admin: Inbox,
  dnd: BellOff,
};

const KIND_LABEL: Record<BlockKind, string> = {
  focus: "Focus",
  errands: "Errands",
  admin: "Admin",
  dnd: "DND",
};

const rowStyle = (_kind: BlockKind): React.CSSProperties => ({
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 10px",
  background: "var(--paper-muted)",
  borderRadius: 6,
  marginBottom: 4,
  cursor: "pointer",
  fontSize: 13,
});


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
      <PageHeader icon={LayoutGrid} title="Time blocks" />

      <section style={sectionStyle}>
        <SectionLabel icon={LayoutGrid}>This week</SectionLabel>
        {weekBlocks.length === 0 ? (
          <p style={{ color: "var(--ink-faint)", fontSize: 13, margin: 0 }}>No blocks this week yet.</p>
        ) : (
          DAYS.map((day) => {
            const bs = byDay[day];
            if (bs.length === 0) return null;
            return (
              <div key={day}>
                <div style={dayHeading}>{day}</div>
                {[...bs].sort((a, b) => a.start_time.localeCompare(b.start_time)).map((b) => (
                  <div key={b.id} style={rowStyle(b.kind)} onClick={() => setEditing(b)}>
                    {(() => { const Icon = KIND_ICON[b.kind] ?? Target; return <Icon size={14} strokeWidth={1.8} color="var(--ink-soft)" aria-label={KIND_LABEL[b.kind]} />; })()}
                    <span style={{ flex: 1 }}>{b.title}</span>
                    <span style={{ color: "var(--ink-faint)", fontSize: 12 }}>
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
        <SectionLabel icon={RefreshCw}>Recurring patterns</SectionLabel>
        {recurring.length === 0 ? (
          <p style={{ color: "var(--ink-faint)", fontSize: 13, margin: 0 }}>No patterns yet. Nell will suggest one when she notices a repetition.</p>
        ) : (
          recurring.map((b) => (
            <div key={b.id} style={rowStyle(b.kind)} onClick={() => setEditing(b)}>
              {(() => { const Icon = KIND_ICON[b.kind] ?? Target; return <Icon size={14} strokeWidth={1.8} color="var(--ink-soft)" aria-label={KIND_LABEL[b.kind]} />; })()}
              <span style={{ flex: 1 }}>{b.title}</span>
              <span style={{ color: "var(--ink-faint)", fontSize: 12 }}>
                {b.rrule ? rruleToEnglish(b.rrule) : ""} · {b.start_time}–{b.end_time}
              </span>
            </div>
          ))
        )}
      </section>

      <Button variant="primary" icon={Plus} onClick={() => setCreating(true)}>Add block</Button>

      {creating && <BlockDrawer block={null} onClose={() => setCreating(false)} />}
      {editing && <BlockDrawer block={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}
