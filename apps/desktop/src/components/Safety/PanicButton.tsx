import { useState } from "react";
import { AlertTriangle, X, Trash2 } from "lucide-react";
import { panicEraseEverything } from "../../lib/safety/ipc";
import {
  COLOR_DANGER,
  TEXT_SECONDARY,
  dangerButton,
  settingsStatusDanger,
} from "../Settings/styles";
import { Button } from "../../lib/ui";

export default function PanicButton() {
  const [phase, setPhase] = useState<"idle" | "confirming" | "erasing" | "done" | "error">(
    "idle",
  );
  const [typed, setTyped] = useState("");
  const [err, setErr] = useState<string | null>(null);

  const run = async () => {
    setPhase("erasing");
    setErr(null);
    try {
      await panicEraseEverything(typed);
      setPhase("done");
    } catch (e) {
      setErr(String(e));
      setPhase("error");
    }
  };

  if (phase === "done") {
    return (
      <div style={{ ...settingsStatusDanger, padding: 16 }}>
        <strong style={{ color: COLOR_DANGER }}>Everything erased.</strong>
        <div style={{ fontSize: 13, marginTop: 4, color: "var(--ink)" }}>
          Quit Manor and reopen to start fresh.
        </div>
      </div>
    );
  }

  return (
    <section style={{ ...settingsStatusDanger, padding: 16 }}>
      <h2 style={{ margin: 0, color: COLOR_DANGER, fontSize: 15 }}>Erase everything</h2>
      <p style={{ fontSize: 13, color: TEXT_SECONDARY, marginTop: 4, marginBottom: 8 }}>
        Deletes the Manor database and every attachment on this Mac. Backup files elsewhere
        are not affected. There is no undo.
      </p>
      {phase === "idle" && (
        <Button
          variant="secondary"
          icon={AlertTriangle}
          onClick={() => setPhase("confirming")}
          style={{ color: COLOR_DANGER, borderColor: COLOR_DANGER }}
        >
          I understand — let me erase
        </Button>
      )}
      {phase === "confirming" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <label style={{ fontSize: 13, color: "var(--ink)" }}>
            Type{" "}
            <code
              style={{
                fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                fontSize: 12,
                background: "var(--paper-muted)",
                padding: "1px 4px",
                borderRadius: 3,
              }}
            >
              DELETE
            </code>{" "}
            to confirm:
            <input
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              style={{ marginLeft: 6, fontFamily: "monospace" }}
            />
          </label>
          <div style={{ display: "flex", gap: 6 }}>
            <Button
              variant="secondary"
              icon={X}
              onClick={() => {
                setPhase("idle");
                setTyped("");
              }}
            >
              Cancel
            </Button>
            <Button icon={Trash2} onClick={run} disabled={typed !== "DELETE"} style={dangerButton}>
              Erase everything
            </Button>
          </div>
        </div>
      )}
      {phase === "erasing" && <div style={{ color: "var(--ink)" }}>Erasing…</div>}
      {phase === "error" && (
        <div style={{ color: COLOR_DANGER, fontSize: 13 }}>Failed: {err}</div>
      )}
    </section>
  );
}
