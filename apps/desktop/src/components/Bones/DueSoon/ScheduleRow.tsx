import { Wrench, MoreHorizontal } from "lucide-react";
import type { MaintenanceSchedule } from "../../../lib/maintenance/ipc";

interface Props {
  schedule: MaintenanceSchedule;
  assetName?: string;
  onMarkDone: () => void;
  onEdit: () => void;
  onDelete?: () => void;
}

function todayIso(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function formatRelativeDue(nextDueDate: string): string {
  const today = new Date(todayIso() + "T00:00:00");
  const due = new Date(nextDueDate + "T00:00:00");
  const diffDays = Math.round((due.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
  if (diffDays < 0) {
    const n = -diffDays;
    return `${n} day${n === 1 ? "" : "s"} overdue`;
  }
  if (diffDays === 0) return "due today";
  if (diffDays === 1) return "due tomorrow";
  if (diffDays <= 30) return `due in ${diffDays} days`;
  const weeks = Math.round(diffDays / 7);
  return `due in ${weeks} weeks`;
}

export function ScheduleRow({ schedule, assetName, onMarkDone, onEdit, onDelete }: Props) {
  const isOverdue = formatRelativeDue(schedule.next_due_date).includes("overdue")
    || formatRelativeDue(schedule.next_due_date) === "due today";
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 12,
      padding: "10px 12px",
      borderBottom: "1px solid var(--hairline, #e5e5e5)",
    }}>
      <Wrench size={16} strokeWidth={1.8} color="var(--ink-soft, #999)" />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 600, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {schedule.task}
        </div>
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
          {assetName ? `${assetName} · ` : ""}
          <span style={{ color: isOverdue ? "var(--ink-danger, #b00020)" : undefined }}>
            {formatRelativeDue(schedule.next_due_date)}
          </span>
        </div>
      </div>
      <button type="button" onClick={onMarkDone}>Mark done</button>
      <button type="button" onClick={onEdit} aria-label="Edit schedule">
        <MoreHorizontal size={14} strokeWidth={1.8} />
      </button>
      {onDelete && (
        <button type="button" onClick={onDelete} aria-label="Delete schedule"
          style={{ background: "transparent", border: "none", cursor: "pointer" }}>
          ✕
        </button>
      )}
    </div>
  );
}
