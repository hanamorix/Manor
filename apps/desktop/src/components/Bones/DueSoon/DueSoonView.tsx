import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { useMaintenanceStore } from "../../../lib/maintenance/state";
import type { ScheduleWithAsset, MaintenanceSchedule } from "../../../lib/maintenance/ipc";
import { ScheduleRow } from "./ScheduleRow";
import { ScheduleDrawer } from "./ScheduleDrawer";    // Task 9

function todayIso(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function classifyBand(nextDueDate: string): "overdue" | "this_week" | "upcoming" | "far" {
  const today = new Date(todayIso() + "T00:00:00");
  const due = new Date(nextDueDate + "T00:00:00");
  const days = Math.round((due.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
  if (days <= 0) return "overdue";
  if (days <= 7) return "this_week";
  if (days <= 30) return "upcoming";
  return "far";
}

export function DueSoonView() {
  const { dueSoon, loadStatus, loadDueSoon, markDone, deleteSchedule } = useMaintenanceStore();
  const [editing, setEditing] = useState<MaintenanceSchedule | null>(null);
  const [adding, setAdding] = useState(false);

  useEffect(() => { void loadDueSoon(); }, [loadDueSoon]);

  const overdue = dueSoon.filter((s) => classifyBand(s.schedule.next_due_date) === "overdue");
  const thisWeek = dueSoon.filter((s) => classifyBand(s.schedule.next_due_date) === "this_week");
  const upcoming = dueSoon.filter((s) => classifyBand(s.schedule.next_due_date) === "upcoming");

  const renderBand = (title: string, rows: ScheduleWithAsset[]) => {
    if (rows.length === 0) return null;
    return (
      <div style={{ marginBottom: 24 }}>
        <h2 style={{ fontSize: 14, fontWeight: 600, marginBottom: 8,
                     color: title === "Overdue" ? "var(--ink-danger, #b00020)" : undefined }}>
          {title}  <span style={{ color: "var(--ink-soft, #999)", fontWeight: 500 }}>({rows.length})</span>
        </h2>
        <div style={{ border: "1px solid var(--hairline, #e5e5e5)", borderRadius: 6, overflow: "hidden" }}>
          {rows.map((r) => (
            <ScheduleRow
              key={r.schedule.id}
              schedule={r.schedule}
              assetName={r.asset_name}
              onMarkDone={() => void markDone(r.schedule.id)}
              onEdit={() => setEditing(r.schedule)}
            />
          ))}
        </div>
      </div>
    );
  };

  const allEmpty = dueSoon.length === 0;

  return (
    <div>
      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && (
        <p style={{ color: "var(--ink-danger, #b00020)" }}>
          {loadStatus.message} — <button onClick={() => void loadDueSoon()}>Retry</button>
        </p>
      )}

      {loadStatus.kind === "idle" && allEmpty && (
        <div style={{ padding: 48, textAlign: "center" }}>
          <p style={{ color: "var(--ink-soft, #999)", marginBottom: 16 }}>
            Nothing due in the next 30 days. Everything in order.
          </p>
          <button onClick={() => setAdding(true)}
            style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
            <Plus size={14} strokeWidth={1.8} /> New schedule
          </button>
        </div>
      )}

      {loadStatus.kind === "idle" && !allEmpty && (
        <>
          {renderBand("Overdue", overdue)}
          {renderBand("Due this week", thisWeek)}
          {renderBand("Upcoming (next 30 days)", upcoming)}
          <div style={{ marginTop: 24, textAlign: "right" }}>
            <button onClick={() => setAdding(true)}
              style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
              <Plus size={14} strokeWidth={1.8} /> New schedule
            </button>
          </div>
        </>
      )}

      {adding && (
        <ScheduleDrawer
          onClose={() => setAdding(false)}
          onSaved={() => { setAdding(false); void loadDueSoon(); }}
        />
      )}
      {editing && (
        <ScheduleDrawer
          schedule={editing}
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); void loadDueSoon(); }}
          onDeleted={() => { void deleteSchedule(editing.id); setEditing(null); }}
        />
      )}
    </div>
  );
}
