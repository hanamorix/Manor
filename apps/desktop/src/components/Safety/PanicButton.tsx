import { useState } from "react";
import { panicEraseEverything } from "../../lib/safety/ipc";

export default function PanicButton() {
  const [phase, setPhase] = useState<"idle" | "confirming" | "erasing" | "done" | "error">("idle");
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
      <div style={{ padding: 16, background: "#130808", border: "1px solid #c33", borderRadius: 6 }}>
        <strong>Everything erased.</strong>
        <div style={{ fontSize: 13, marginTop: 4 }}>Quit Manor and reopen to start fresh.</div>
      </div>
    );
  }

  return (
    <section style={{ padding: 16, border: "1px solid #552", borderRadius: 6, background: "#1a1410" }}>
      <h2 style={{ margin: 0, color: "#d90" }}>Erase everything</h2>
      <p style={{ fontSize: 13, color: "#aaa" }}>
        Deletes the Manor database and every attachment on this Mac. Backup files
        elsewhere are not affected. There is no undo.
      </p>
      {phase === "idle" && (
        <button onClick={() => setPhase("confirming")} style={{ color: "#f66" }}>
          I understand — let me erase
        </button>
      )}
      {phase === "confirming" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <label style={{ fontSize: 13 }}>Type <code>DELETE</code> to confirm:
            <input value={typed} onChange={(e) => setTyped(e.target.value)}
                   style={{ marginLeft: 6, fontFamily: "monospace" }} />
          </label>
          <div style={{ display: "flex", gap: 6 }}>
            <button onClick={() => { setPhase("idle"); setTyped(""); }}>Cancel</button>
            <button onClick={run} disabled={typed !== "DELETE"}
                    style={{ background: "#c33", color: "#fff" }}>
              Erase everything
            </button>
          </div>
        </div>
      )}
      {phase === "erasing" && <div>Erasing…</div>}
      {phase === "error" && (
        <div style={{ color: "#f66", fontSize: 13 }}>Failed: {err}</div>
      )}
    </section>
  );
}
