import { useEffect, useState } from "react";
import { ping, type PingResponse } from "./lib/ipc";

type State =
  | { kind: "loading" }
  | { kind: "ok"; resp: PingResponse }
  | { kind: "error"; message: string };

export default function App() {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    ping()
      .then((resp) => setState({ kind: "ok", resp }))
      .catch((e: unknown) =>
        setState({ kind: "error", message: String(e) }),
      );
  }, []);

  return (
    <main style={{ padding: "2rem" }}>
      <h1>Manor</h1>
      {state.kind === "loading" && <p>Contacting core…</p>}
      {state.kind === "ok" && (
        <p>
          Core says <strong>{state.resp.message}</strong> (core version{" "}
          {state.resp.core_version}).
        </p>
      )}
      {state.kind === "error" && (
        <p style={{ color: "#b91c1c" }}>Error: {state.message}</p>
      )}
    </main>
  );
}
