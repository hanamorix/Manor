import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";
import { getRenewalAlerts } from "../../lib/ledger/ipc";

export default function RenewalAlertsCard() {
  const { renewalAlerts, setRenewalAlerts } = useTodayStore();

  useEffect(() => {
    void getRenewalAlerts().then(setRenewalAlerts).catch(() => {});
  }, [setRenewalAlerts]);

  if (renewalAlerts.length === 0) return null;

  return (
    <section
      style={{
        background: "#fff",
        border: "1px solid rgba(255, 149, 0, 0.35)",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--shadow-sm)",
        padding: 12,
      }}
    >
      <header
        style={{
          fontSize: 11,
          color: "#b36b00",
          marginBottom: 8,
          textTransform: "uppercase",
          letterSpacing: 0.5,
          fontWeight: 700,
        }}
      >
        What matters
      </header>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {renewalAlerts.map((a) => {
          const isRed = a.severity === "red";
          return (
            <div
              key={a.contract_id}
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                padding: 8,
                borderRadius: "var(--radius-sm)",
                background: isRed
                  ? "rgba(255, 59, 48, 0.08)"
                  : "rgba(255, 149, 0, 0.08)",
                border: isRed
                  ? "1px solid rgba(255, 59, 48, 0.25)"
                  : "1px solid rgba(255, 149, 0, 0.25)",
              }}
            >
              <div style={{ fontSize: 13, color: "var(--ink)" }}>
                {a.provider}{" "}
                <span style={{ fontSize: 11, color: "rgba(0,0,0,0.55)" }}>
                  ({a.kind})
                </span>{" "}
                renewing soon
              </div>
              <span
                style={{
                  padding: "2px 10px",
                  borderRadius: "var(--radius-pill)",
                  fontSize: 11,
                  fontWeight: 600,
                  background: isRed ? "var(--imessage-red)" : "#b36b00",
                  color: "#fff",
                }}
              >
                {a.days_remaining}d
              </span>
            </div>
          );
        })}
      </div>
    </section>
  );
}
