import { useRef, useState } from "react";
import {
  previewCsv,
  importCsv,
  type PreviewRow,
  type ImportResult,
} from "../../lib/ledger/ipc";
import { useLedgerStore } from "../../lib/ledger/state";
import { useOverlay } from "../../lib/overlay/state";

interface Props {
  onClose: () => void;
  onImported: (result: ImportResult) => void;
}

const PRESETS = [
  { id: "monzo", label: "Monzo" },
  { id: "starling", label: "Starling" },
  { id: "barclays", label: "Barclays" },
  { id: "hsbc", label: "HSBC" },
  { id: "natwest", label: "Natwest" },
  { id: "generic", label: "Generic (pick columns)" },
];

const MAX_PREVIEW = 20;

function fmtDate(unix: number): string {
  return new Date(unix * 1000).toLocaleDateString("en-GB", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  });
}

function fmtAmount(pence: number): string {
  const abs = Math.abs(pence) / 100;
  const sign = pence < 0 ? "-" : "+";
  return `${sign}£${abs.toFixed(2)}`;
}

export default function CsvImportDrawer({ onClose, onImported }: Props) {
  useOverlay();

  const categories = useLedgerStore((s) => s.categories);
  const catMap = new Map(categories.map((c) => [c.id, c]));

  const [preset, setPreset] = useState("monzo");
  const [rows, setRows] = useState<PreviewRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const duplicateCount = rows.filter((r) => r.duplicate).length;
  const canImport = rows.length > 0 && !importing;

  async function handleFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;

    setError(null);
    setRows([]);
    setLoading(true);

    try {
      const buf = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buf));
      const result = await previewCsv({ preset, csvBytes: bytes });
      setRows(result.rows);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function doImport() {
    if (!canImport) return;
    setImporting(true);
    setError(null);
    try {
      const result = await importCsv(rows);
      onImported(result);
    } catch (err) {
      setError(String(err));
      setImporting(false);
    }
  }

  const labelStyle: React.CSSProperties = {
    fontSize: 11,
    fontWeight: 700,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    color: "var(--ink-soft)",
    marginBottom: 5,
    display: "block",
  };

  const inputStyle: React.CSSProperties = {
    width: "100%",
    padding: "9px 12px",
    fontSize: 14,
    border: "1px solid var(--hairline)",
    borderRadius: 10,
    background: "var(--hairline)",
    fontFamily: "inherit",
    boxSizing: "border-box",
    color: "var(--ink)",
  };

  const previewRows = rows.slice(0, MAX_PREVIEW);

  return (
    <>
      {/* Backdrop */}
      <div
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          background: "var(--scrim)",
          zIndex: 1050,
        }}
      />

      {/* Drawer */}
      <div
        style={{
          position: "fixed",
          right: 0,
          top: 0,
          bottom: 0,
          width: 600,
          background: "var(--paper)",
          boxShadow: "-4px 0 24px rgba(0,0,0,0.12)",
          zIndex: 1100,
          display: "flex",
          flexDirection: "column",
          animation: "drawerIn 200ms ease-out",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: "18px 20px 14px",
            borderBottom: "1px solid var(--hairline)",
          }}
        >
          <div style={{ fontSize: 16, fontWeight: 700, color: "var(--ink)" }}>
            Import CSV
          </div>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              fontSize: 20,
              cursor: "pointer",
              color: "var(--ink-faint)",
              lineHeight: 1,
              padding: 0,
            }}
          >
            ✕
          </button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {/* Preset selector */}
            <div>
              <label style={labelStyle}>Bank preset</label>
              <select
                style={{ ...inputStyle, appearance: "none" }}
                value={preset}
                onChange={(e) => {
                  setPreset(e.target.value);
                  setRows([]);
                  setError(null);
                  if (fileRef.current) fileRef.current.value = "";
                }}
              >
                {PRESETS.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label}
                  </option>
                ))}
              </select>
            </div>

            {/* File input */}
            <div>
              <label style={labelStyle}>CSV file</label>
              <input
                ref={fileRef}
                style={inputStyle}
                type="file"
                accept=".csv,text/csv"
                onChange={handleFile}
              />
            </div>

            {/* Loading indicator */}
            {loading && (
              <div
                style={{
                  fontSize: 13,
                  color: "var(--paper-muted)",
                  textAlign: "center",
                  padding: "8px 0",
                }}
              >
                Parsing…
              </div>
            )}

            {/* Error banner */}
            {error && (
              <div
                style={{
                  padding: "10px 12px",
                  background: "rgba(255,59,48,0.08)",
                  border: "1px solid rgba(255,59,48,0.3)",
                  borderRadius: 10,
                  fontSize: 13,
                  color: "var(--imessage-red)",
                }}
              >
                {error}
              </div>
            )}

            {/* Preview table */}
            {rows.length > 0 && (
              <div>
                {/* Summary line */}
                <div
                  style={{
                    fontSize: 12,
                    color: "var(--ink-soft)",
                    marginBottom: 8,
                  }}
                >
                  {rows.length} row{rows.length !== 1 ? "s" : ""}
                  {duplicateCount > 0
                    ? ` · ${duplicateCount} duplicate${duplicateCount !== 1 ? "s" : ""}`
                    : ""}
                  {rows.length > MAX_PREVIEW
                    ? ` (showing first ${MAX_PREVIEW})`
                    : ""}
                </div>

                {/* Table */}
                <div
                  style={{
                    border: "1px solid var(--hairline)",
                    borderRadius: 10,
                    overflow: "hidden",
                  }}
                >
                  <table
                    style={{
                      width: "100%",
                      fontSize: 12,
                      borderCollapse: "collapse",
                    }}
                  >
                    <thead>
                      <tr
                        style={{
                          background: "var(--paper-muted)",
                          borderBottom: "1px solid var(--hairline)",
                        }}
                      >
                        {["Date", "Amount", "Description", "Category"].map(
                          (h) => (
                            <th
                              key={h}
                              style={{
                                padding: "7px 10px",
                                textAlign: "left",
                                fontWeight: 700,
                                fontSize: 11,
                                textTransform: "uppercase",
                                letterSpacing: 0.4,
                                color: "var(--ink-soft)",
                                whiteSpace: "nowrap",
                              }}
                            >
                              {h}
                            </th>
                          )
                        )}
                      </tr>
                    </thead>
                    <tbody>
                      {previewRows.map((row, i) => {
                        const cat = row.suggested_category_id != null
                          ? catMap.get(row.suggested_category_id)
                          : undefined;
                        const amountColor = "var(--ink)";

                        return (
                          <tr
                            key={i}
                            style={{
                              borderTop: i > 0 ? "1px solid var(--hairline)" : undefined,
                              opacity: row.duplicate ? 0.4 : 1,
                              background:
                                i % 2 === 0 ? "transparent" : "rgba(0,0,0,0.015)",
                            }}
                          >
                            <td
                              style={{
                                padding: "7px 10px",
                                color: "var(--ink)",
                                whiteSpace: "nowrap",
                              }}
                            >
                              {fmtDate(row.date)}
                            </td>
                            <td
                              style={{
                                padding: "7px 10px",
                                color: amountColor,
                                fontWeight: 600,
                                whiteSpace: "nowrap",
                                fontVariantNumeric: "tabular-nums",
                              }}
                            >
                              {fmtAmount(row.amount_pence)}
                            </td>
                            <td
                              style={{
                                padding: "7px 10px",
                                color: "var(--ink)",
                                maxWidth: 200,
                                overflow: "hidden",
                                textOverflow: "ellipsis",
                                whiteSpace: "nowrap",
                              }}
                            >
                              {row.description}
                            </td>
                            <td
                              style={{
                                padding: "7px 10px",
                                color: cat
                                  ? "var(--ink)"
                                  : "var(--ink-faint)",
                                whiteSpace: "nowrap",
                              }}
                            >
                              {cat ? `${cat.emoji} ${cat.name}` : "—"}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div
          style={{
            padding: "14px 20px",
            borderTop: "1px solid var(--hairline)",
            display: "flex",
            gap: 10,
          }}
        >
          <button
            onClick={onClose}
            style={{
              flex: 1,
              padding: "10px 16px",
              background: "transparent",
              color: "var(--ink)",
              border: "1px solid var(--hairline)",
              borderRadius: "var(--radius-pill)",
              fontSize: 14,
              fontWeight: 600,
              cursor: "pointer",
              fontFamily: "inherit",
            }}
          >
            Cancel
          </button>
          <button
            onClick={doImport}
            disabled={!canImport}
            style={{
              flex: 2,
              padding: "10px 16px",
              background: "var(--ink)",
              color: "var(--action-fg)",
              border: "none",
              borderRadius: "var(--radius-pill)",
              fontSize: 14,
              fontWeight: 600,
              cursor: canImport ? "pointer" : "default",
              opacity: canImport ? 1 : 0.5,
              fontFamily: "inherit",
            }}
          >
            {importing
              ? "Importing…"
              : rows.length > 0
              ? `Import ${rows.length - duplicateCount} transaction${rows.length - duplicateCount !== 1 ? "s" : ""}`
              : "Import"}
          </button>
        </div>
      </div>
    </>
  );
}
