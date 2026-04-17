import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { listContracts, deleteContract } from "../../lib/ledger/ipc";
import { useLedgerStore } from "../../lib/ledger/state";
import type { Contract } from "../../lib/ledger/ipc";
import AddContractDrawer from "./AddContractDrawer";
import { Button } from "../../lib/ui";

function formatPence(pence: number): string {
  return `£${(pence / 100).toFixed(2)}`;
}

function daysUntil(unix: number): number {
  return Math.floor((unix - Date.now() / 1000) / 86400);
}

function kindLabel(kind: Contract["kind"]): string {
  switch (kind) {
    case "phone":      return "Phone";
    case "broadband":  return "Broadband";
    case "insurance":  return "Insurance";
    case "energy":     return "Energy";
    default:           return "Other";
  }
}

interface PillProps {
  days: number;
  alertDays: number;
}

function CountdownPill({ days, alertDays }: PillProps) {
  let bg: string;
  let color: string;
  let label: string;

  if (days < 0) {
    bg = "var(--paper-muted)";
    color = "var(--ink-soft)";
    label = "expired";
  } else if (days <= 7) {
    bg = "var(--paper-muted)";
    color = "var(--ink-danger)";
    label = `${days}d`;
  } else if (days <= alertDays) {
    bg = "var(--paper-muted)";
    color = "var(--ink-soft)";
    label = `${days}d`;
  } else {
    bg = "var(--paper-muted)";
    color = "var(--ink-faint)";
    label = `${days}d`;
  }

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        padding: "2px 8px",
        borderRadius: "var(--radius-pill)",
        background: bg,
        color,
        fontSize: 11,
        fontWeight: 600,
        letterSpacing: 0.3,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </span>
  );
}

export default function ContractsSection() {
  const { contracts, setContracts } = useLedgerStore();
  const [expanded, setExpanded] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [editTarget, setEditTarget] = useState<Contract | null>(null);

  async function refresh() {
    const items = await listContracts();
    setContracts(items);
  }

  useEffect(() => {
    refresh();
  }, []);

  async function handleDelete(id: number, e: React.MouseEvent) {
    e.stopPropagation();
    if (!confirm("Delete this contract?")) return;
    await deleteContract(id);
    await refresh();
  }

  function handleSaved() {
    refresh();
    setShowAdd(false);
    setEditTarget(null);
  }

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
            role="button"
            tabIndex={0}
            onClick={() => setExpanded((v) => !v)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                setExpanded((v) => !v);
              }
            }}
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
              Contracts{" "}
              <span style={{ color: "var(--ink-faint)", fontWeight: 500 }}>
                ({contracts.length})
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
            {contracts.length === 0 ? (
              <div
                style={{
                  padding: "12px 0",
                  textAlign: "center",
                  fontSize: 13,
                  color: "var(--ink-faint)",
                }}
              >
                No contracts yet.
              </div>
            ) : (
              contracts.map((c) => {
                const days = daysUntil(c.term_end);
                return (
                  <div
                    key={c.id}
                    onClick={() => setEditTarget(c)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      padding: "8px 10px",
                      borderRadius: "var(--radius-sm)",
                      background: "var(--surface)",
                      border: "1px solid var(--hairline)",
                      cursor: "pointer",
                      transition: "opacity 150ms",
                    }}
                  >
                    {/* Left: provider + kind */}
                    <div style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
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
                        {c.provider}
                      </div>
                      <div
                        style={{
                          fontSize: 11,
                          color: "var(--ink-faint)",
                          marginTop: 1,
                        }}
                      >
                        {kindLabel(c.kind)}
                        {c.description ? ` · ${c.description}` : ""}
                      </div>
                    </div>

                    {/* Right: cost + pill + edit + delete */}
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 8,
                        flexShrink: 0,
                      }}
                    >
                      <span
                        style={{ fontSize: 13, fontWeight: 600, color: "var(--ink)" }}
                      >
                        {formatPence(c.monthly_cost_pence)}/mo
                      </span>
                      <CountdownPill days={days} alertDays={c.renewal_alert_days} />
                      <button
                        onClick={(e) => { e.stopPropagation(); setEditTarget(c); }}
                        title="Edit"
                        style={{
                          background: "none",
                          border: "none",
                          fontSize: 13,
                          cursor: "pointer",
                          color: "var(--ink-faint)",
                          padding: "2px 4px",
                          lineHeight: 1,
                          fontFamily: "inherit",
                        }}
                      >
                        ✎
                      </button>
                      <button
                        onClick={(e) => handleDelete(c.id, e)}
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
        <AddContractDrawer
          onClose={() => setShowAdd(false)}
          onSaved={handleSaved}
        />
      )}

      {/* Edit drawer */}
      {editTarget && (
        <AddContractDrawer
          existing={editTarget}
          onClose={() => setEditTarget(null)}
          onSaved={handleSaved}
        />
      )}
    </>
  );
}
