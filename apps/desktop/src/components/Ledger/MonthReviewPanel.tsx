import { useState } from "react";
import { BookOpen } from "lucide-react";
import { aiMonthReview, type MonthlySummary, type StreamChunk } from "../../lib/ledger/ipc";
import { Button } from "../../lib/ui";

interface Props {
  year: number;
  month: number;
  summary: MonthlySummary;
}

export default function MonthReviewPanel({ year, month, summary }: Props) {
  const [text, setText] = useState("");
  const [running, setRunning] = useState(false);
  const [refreshedAt, setRefreshedAt] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);

  const net = summary.total_in_pence - summary.total_out_pence;

  const run = async () => {
    setRunning(true);
    setText("");
    setError(null);
    try {
      await aiMonthReview({ year, month }, (c: StreamChunk) => {
        if (c.type === "Token") setText((t) => t + c.data);
        else if (c.type === "Error") setError(c.data);
      });
      setRefreshedAt(new Date());
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <section
      style={{
        background: "var(--paper)",
        border: "1px solid var(--hairline)",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--shadow-sm)",
        padding: 16,
      }}
    >
      {/* Summary row */}
      <div style={{ display: "flex", gap: 24, marginBottom: text ? 16 : 0 }}>
        <div>
          <div style={{ fontSize: 11, color: "var(--ink-soft)", marginBottom: 6 }}>In</div>
          <div style={{ fontSize: 18, fontWeight: 600, color: "var(--ink)" }}>
            {"+ "}£{(summary.total_in_pence / 100).toFixed(2)}
          </div>
        </div>
        <div>
          <div style={{ fontSize: 11, color: "var(--ink-soft)", marginBottom: 6 }}>Out</div>
          <div style={{ fontSize: 18, fontWeight: 600, color: "var(--ink)" }}>
            {"\u2212 "}£{(summary.total_out_pence / 100).toFixed(2)}
          </div>
        </div>
        <div>
          <div style={{ fontSize: 11, color: "var(--ink-soft)", marginBottom: 6 }}>Net</div>
          <div
            style={{
              fontSize: 18,
              fontWeight: 700,
              color: "var(--ink)",
            }}
          >
            {"= "}£{(net / 100).toFixed(2)}
          </div>
        </div>
      </div>

      {/* Review with AI section */}
      {text ? (
        <>
          <div style={{ whiteSpace: "pre-wrap", fontSize: 14, color: "var(--ink)", marginBottom: 12 }}>
            {text}
          </div>
          {refreshedAt && (
            <div style={{ fontSize: 11, color: "var(--ink-soft)" }}>
              Refreshed {refreshedAt.toLocaleTimeString()} ·{" "}
              <a
                href="#"
                onClick={(e) => {
                  e.preventDefault();
                  void run();
                }}
                style={{
                  color: "var(--ink)",
                  textDecoration: "none",
                  fontWeight: 600,
                }}
              >
                Refresh
              </a>
            </div>
          )}
        </>
      ) : (
        <Button variant="primary" icon={BookOpen} onClick={run} disabled={running} style={{ opacity: running ? 0.6 : 1 }}>
          {running ? "Thinking…" : "Review with AI"}
        </Button>
      )}

      {error && (
        <div
          style={{
            marginTop: 12,
            padding: "8px 12px",
            fontSize: 12,
            color: "var(--ink)",
            background: "rgba(255, 59, 48, 0.06)",
            borderRadius: "var(--radius-sm)",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <span>AI unavailable — start Ollama to use this feature.</span>
          <button
            onClick={() => setError(null)}
            style={{
              background: "none",
              border: "none",
              color: "var(--ink-soft)",
              fontSize: 16,
              cursor: "pointer",
              padding: "2px 6px",
              lineHeight: 1,
            }}
          >
            ×
          </button>
        </div>
      )}
    </section>
  );
}
