import { Pencil, Link as LinkIcon } from "lucide-react";
import type { EventWithContext } from "../../lib/maintenance/event-ipc";

interface Props {
  row: EventWithContext;
  onEdit(): void;
}

function formatGBP(pence: number): string {
  return `£${(pence / 100).toFixed(2)}`;
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("en-GB", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function EventRow({ row, onEdit }: Props) {
  const { event, transaction_description, schedule_deleted } = row;
  const showBackfillPill = event.source === "backfill" && event.cost_pence === null;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "8px 0",
        borderBottom: "1px solid var(--border, #eee)",
      }}
    >
      <span style={{ color: "var(--ink-soft, #777)", fontSize: 13, minWidth: 96 }}>
        {formatDate(event.completed_date)}
      </span>
      <span style={{ flex: 1 }}>
        {event.title}
        {schedule_deleted && (
          <span
            style={{ color: "var(--ink-soft, #999)", fontSize: 12, marginLeft: 6 }}
          >
            (schedule removed)
          </span>
        )}
      </span>
      {event.cost_pence !== null && (
        <span style={{ fontVariantNumeric: "tabular-nums" }}>
          {formatGBP(event.cost_pence)}
        </span>
      )}
      {transaction_description && (
        <span
          title={transaction_description}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            fontSize: 12,
            padding: "2px 6px",
            borderRadius: 4,
            background: "var(--surface-subtle, #f4f4f4)",
          }}
        >
          <LinkIcon size={12} strokeWidth={1.6} />
          {transaction_description.slice(0, 18)}
        </span>
      )}
      {showBackfillPill && (
        <span
          style={{
            fontSize: 11,
            color: "var(--ink-soft, #999)",
            padding: "2px 6px",
            border: "1px solid var(--border, #ddd)",
            borderRadius: 4,
          }}
        >
          backfill
        </span>
      )}
      <button
        type="button"
        onClick={onEdit}
        aria-label="Edit event"
        style={{ background: "none", border: "none", cursor: "pointer" }}
      >
        <Pencil size={14} strokeWidth={1.6} />
      </button>
    </div>
  );
}
