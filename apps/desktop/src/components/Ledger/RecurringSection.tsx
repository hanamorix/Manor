import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { listRecurring, deleteRecurring } from "../../lib/ledger/ipc";
import { useLedgerStore } from "../../lib/ledger/state";
import type { Category, RecurringPayment } from "../../lib/ledger/ipc";
import AddRecurringDrawer from "./AddRecurringDrawer";
import { Button } from "../../lib/ui";

interface Props {
  categories: Category[];
}

function formatPence(pence: number): string {
  return `£${(pence / 100).toFixed(2)}`;
}

export default function RecurringSection({ categories }: Props) {
  const { recurring, setRecurring } = useLedgerStore();
  const [expanded, setExpanded] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [editTarget, setEditTarget] = useState<RecurringPayment | null>(null);

  async function refresh() {
    const items = await listRecurring();
    setRecurring(items);
  }

  useEffect(() => {
    refresh();
  }, []);

  async function handleDelete(id: number, e: React.MouseEvent) {
    e.stopPropagation();
    if (!confirm("Delete this recurring payment?")) return;
    await deleteRecurring(id);
    await refresh();
  }

  function handleSaved() {
    refresh();
    setShowAdd(false);
    setEditTarget(null);
  }

  const activeCount = recurring.filter((r) => r.active).length;

  // Build a map of category id → emoji for quick lookup
  const catMap = new Map(categories.map((c) => [c.id, c]));

  return (
    <>
      <div
        style={{
          background: "var(--paper)",
          border: "1px solid var(--hairline)",
          borderRadius: "var(--radius-md)",
          padding: 12,
          marginBottom: 16,
        }}
      >
        {/* Section header */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: expanded ? 10 : 0,
          }}
        >
          <div
            onClick={() => setExpanded((v) => !v)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              cursor: "pointer",
              userSelect: "none",
            }}
          >
            <span style={{ fontSize: 13, color: "var(--ink-faint)" }}>
              {expanded ? "▾" : "▸"}
            </span>
            <h3
              style={{
                margin: 0,
                fontSize: 14,
                fontWeight: 600,
                color: "var(--ink)",
              }}
            >
              Recurring{" "}
              <span style={{ color: "var(--ink-faint)", fontWeight: 500 }}>
                ({activeCount} active)
              </span>
            </h3>
          </div>
          <Button variant="primary" icon={Plus} onClick={() => setShowAdd(true)}>
            Add
          </Button>
        </div>

        {/* Row list */}
        {expanded && (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {recurring.length === 0 ? (
              <div
                style={{
                  padding: "12px 0",
                  textAlign: "center",
                  fontSize: 13,
                  color: "var(--ink-faint)",
                }}
              >
                No recurring payments yet.
              </div>
            ) : (
              recurring.map((r) => {
                const cat = r.category_id != null ? catMap.get(r.category_id) : null;
                return (
                  <div
                    key={r.id}
                    onClick={() => setEditTarget(r)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      padding: "8px 10px",
                      borderRadius: "var(--radius-sm)",
                      background: "var(--paper-muted)",
                      border: "1px solid var(--hairline)",
                      cursor: "pointer",
                      opacity: r.active ? 1 : 0.6,
                      transition: "opacity 150ms",
                    }}
                  >
                    {/* Left: emoji + info */}
                    <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
                      <span style={{ fontSize: 18 }}>{cat?.emoji ?? "🔄"}</span>
                      <div style={{ minWidth: 0 }}>
                        <div
                          style={{
                            fontSize: 13,
                            fontWeight: 600,
                            color: "var(--ink)",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {r.description}
                        </div>
                        <div
                          style={{
                            fontSize: 11,
                            color: "var(--ink-faint)",
                            marginTop: 1,
                          }}
                        >
                          {cat ? cat.name : "Uncategorised"} · day {r.day_of_month}
                          {!r.active && " · paused"}
                        </div>
                      </div>
                    </div>

                    {/* Right: amount + delete */}
                    <div
                      style={{ display: "flex", alignItems: "center", gap: 8, flexShrink: 0 }}
                    >
                      <span
                        style={{ fontSize: 13, fontWeight: 600, color: "var(--ink)" }}
                      >
                        {formatPence(r.amount_pence)}
                      </span>
                      <button
                        onClick={(e) => handleDelete(r.id, e)}
                        title="Delete"
                        style={{
                          background: "none",
                          border: "none",
                          fontSize: 14,
                          cursor: "pointer",
                          color: "var(--ink-faint)",
                          padding: "2px 4px",
                          lineHeight: 1,
                          fontFamily: "inherit",
                        }}
                      >
                        ✕
                      </button>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        )}
      </div>

      {/* Add drawer */}
      {showAdd && (
        <AddRecurringDrawer
          categories={categories}
          onClose={() => setShowAdd(false)}
          onSaved={handleSaved}
        />
      )}

      {/* Edit drawer */}
      {editTarget && (
        <AddRecurringDrawer
          categories={categories}
          existing={editTarget}
          onClose={() => setEditTarget(null)}
          onSaved={handleSaved}
        />
      )}
    </>
  );
}
