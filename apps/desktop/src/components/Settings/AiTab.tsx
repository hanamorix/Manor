import { useEffect, useState } from "react";
import { ollamaStatus, type OllamaStatus } from "../../lib/settings/ipc";
import { settingGet, settingSet } from "../../lib/foundation/ipc";

const DEFAULT_MODEL_KEY = "ai.default_model";

export default function AiTab() {
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string>("");

  useEffect(() => {
    void (async () => {
      setLoading(true);
      const s = await ollamaStatus().catch(() => ({ reachable: false, models: [] } as OllamaStatus));
      setStatus(s);
      const stored = await settingGet(DEFAULT_MODEL_KEY);
      setSelected(stored ?? "qwen2.5:7b-instruct");
      setLoading(false);
    })();
  }, []);

  const onModelChange = async (v: string) => {
    setSelected(v);
    await settingSet(DEFAULT_MODEL_KEY, v);
  };

  return (
    <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 16 }}>
      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Local brain (Ollama)</h2>
        {loading && <div style={{ fontSize: 13 }}>Checking…</div>}
        {!loading && status && (
          status.reachable ? (
            <>
              <div style={{ fontSize: 13 }}>
                <span style={{ color: "#6f6" }}>● Ready</span>{" "}
                <span style={{ color: "#888" }}>— {status.models.length} model(s) installed</span>
              </div>
              <label style={{ display: "block", marginTop: 8, fontSize: 13 }}>
                Default model:
                <select
                  value={selected}
                  onChange={(e) => void onModelChange(e.target.value)}
                  style={{ marginLeft: 8 }}
                >
                  {status.models.length === 0 && <option value="">(no models)</option>}
                  {status.models.map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              </label>
            </>
          ) : (
            <div>
              <div style={{ fontSize: 13, color: "#f66" }}>● Unreachable</div>
              <div style={{ fontSize: 12, color: "#888", marginTop: 4 }}>
                Start Ollama to enable chat, CalDAV AI review, and embeddings.{" "}
                <a href="https://ollama.com/download" target="_blank" rel="noreferrer">Install Ollama →</a>
              </div>
            </div>
          )
        )}
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Embeddings</h2>
        <div style={{ fontSize: 12, color: "#888" }}>
          Local semantic search indexing. Available once Phase C ships.
        </div>
        <button disabled style={{ marginTop: 6, fontSize: 12 }}>Rebuild embeddings</button>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Remote providers</h2>
        <div style={{ fontSize: 12, color: "#888" }}>
          Bring-your-own-key providers (Claude, OpenAI, Gemini) will appear here in a future release.
        </div>
      </section>
    </div>
  );
}
