import { useEffect, useState } from "react";
import {
  ollamaStatus,
  type OllamaStatus,
  embeddingsStatus,
  embeddingsRebuild,
  type EmbeddingsStatus,
} from "../../lib/settings/ipc";
import { settingGet, settingSet } from "../../lib/foundation/ipc";
import {
  remoteProviderStatus,
  remoteSetKey,
  remoteRemoveKey,
  remoteSetBudget,
  remoteSetEnabledForReview,
  remoteTest,
  remoteCallLogList,
  remoteCallLogClear,
  type RemoteProviderStatus,
  type CallLogEntry,
} from "../../lib/remote/ipc";
import {
  COLOR_AMBER,
  COLOR_DANGER,
  COLOR_SUCCESS,
  TEXT_MUTED,
  TEXT_SECONDARY,
  settingsCard,
  settingsCodeBlock,
  settingsListRow,
  settingsStatusGood,
  settingsStatusWarn,
} from "./styles";

const DEFAULT_MODEL_KEY = "ai.default_model";

function RemoteProvidersSection() {
  const [status, setStatus] = useState<RemoteProviderStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [newKey, setNewKey] = useState("");
  const [budgetInput, setBudgetInput] = useState("");
  const [testing, setTesting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    try {
      const s = await remoteProviderStatus();
      setStatus(s);
      setBudgetInput((s.budget_pence / 100).toFixed(2));
    } catch {
      setStatus(null);
    }
    setLoading(false);
  };

  useEffect(() => {
    void refresh();
  }, []);

  const saveKey = async () => {
    if (!newKey.trim()) return;
    setMessage(null);
    try {
      await remoteSetKey("claude", newKey.trim());
      setNewKey("");
      setMessage("Key stored in macOS Keychain.");
      await refresh();
    } catch (e) {
      setMessage(`Failed: ${e}`);
    }
  };

  const removeKey = async () => {
    if (!confirm("Remove the Claude API key from Keychain?")) return;
    await remoteRemoveKey("claude");
    setMessage("Key removed.");
    await refresh();
  };

  const saveBudget = async () => {
    const pence = Math.round(parseFloat(budgetInput) * 100);
    if (isNaN(pence) || pence < 0) {
      setMessage("Budget must be a non-negative number.");
      return;
    }
    try {
      await remoteSetBudget("claude", pence);
      setMessage("Budget saved.");
      await refresh();
    } catch (e) {
      setMessage(`Failed: ${e}`);
    }
  };

  const toggleEnabled = async (next: boolean) => {
    try {
      await remoteSetEnabledForReview(next);
      await refresh();
    } catch (e) {
      setMessage(`Failed: ${e}`);
    }
  };

  const test = async () => {
    setTesting(true);
    setMessage(null);
    try {
      const text = await remoteTest();
      setMessage(`Test call succeeded: "${text}"`);
      await refresh();
    } catch (e) {
      setMessage(`Test failed: ${e}`);
    }
    setTesting(false);
  };

  if (loading)
    return (
      <section>
        <div style={{ fontSize: "var(--text-sm)", color: TEXT_MUTED }}>Loading remote providers…</div>
      </section>
    );
  if (!status)
    return (
      <section>
        <div style={{ fontSize: "var(--text-sm)", color: COLOR_DANGER }}>Failed to load remote status.</div>
      </section>
    );

  const pct =
    status.budget_pence > 0
      ? Math.min(100, (status.spent_month_pence / status.budget_pence) * 100)
      : 0;
  const barColor =
    pct >= 100 ? COLOR_DANGER : pct >= 75 ? COLOR_AMBER : COLOR_SUCCESS;

  return (
    <section>
      <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>
        Remote providers
      </h2>

      <div style={{ ...settingsCard, marginBottom: 10 }}>
        <div
          style={{ fontSize: "var(--text-sm)", fontWeight: 600, marginBottom: 4, color: "var(--ink)" }}
        >
          Claude
        </div>
        {status.has_key ? (
          <div
            style={{
              fontSize: "var(--text-xs)",
              color: COLOR_SUCCESS,
              display: "flex",
              alignItems: "center",
            }}
          >
            ● API key set in Keychain
            <button onClick={removeKey} style={{ marginLeft: 8, fontSize: 11 }}>
              Remove
            </button>
          </div>
        ) : (
          <div style={{ display: "flex", gap: 6, marginTop: 4 }}>
            <input
              type="password"
              value={newKey}
              onChange={(e) => setNewKey(e.target.value)}
              placeholder="sk-ant-..."
              style={{ flex: 1, fontSize: 12 }}
            />
            <button onClick={saveKey} disabled={!newKey.trim()} style={{ fontSize: 12 }}>
              Set key
            </button>
          </div>
        )}
      </div>

      <div style={{ marginBottom: 10 }}>
        <div style={{ fontSize: "var(--text-xs)", color: TEXT_MUTED, marginBottom: 4 }}>
          Monthly budget: £{(status.spent_month_pence / 100).toFixed(2)} spent of £
          {(status.budget_pence / 100).toFixed(2)}
        </div>
        <div
          style={{
            height: 6,
            background: "var(--hairline)",
            borderRadius: 3,
            overflow: "hidden",
          }}
        >
          <div
            style={{
              width: "100%",
              transform: `scaleX(${Math.min(pct, 100) / 100})`,
              transformOrigin: "left",
              height: "100%",
              background: barColor,
              transition: "transform var(--duration-med) var(--ease-out)",
            }}
          />
        </div>
        <div style={{ display: "flex", gap: 6, marginTop: 4, alignItems: "center" }}>
          <span style={{ fontSize: "var(--text-xs)", color: TEXT_SECONDARY }}>£</span>
          <input
            type="number"
            step="0.01"
            min="0"
            value={budgetInput}
            onChange={(e) => setBudgetInput(e.target.value)}
            style={{ width: 80, fontSize: 12 }}
          />
          <button onClick={saveBudget} style={{ fontSize: 12 }}>
            Save budget
          </button>
        </div>
      </div>

      <label
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          fontSize: "var(--text-sm)",
          marginBottom: 8,
          color: "var(--ink)",
        }}
      >
        <input
          type="checkbox"
          checked={status.enabled_for_review}
          onChange={(e) => void toggleEnabled(e.target.checked)}
          disabled={!status.has_key}
        />
        Use Claude for ledger AI month review
      </label>

      {status.has_key && (
        <button onClick={test} disabled={testing} style={{ fontSize: 12 }}>
          {testing ? "Testing…" : "Test connection"}
        </button>
      )}

      {message && (
        <div
          style={{
            fontSize: 11,
            color: message.includes("ailed") ? COLOR_DANGER : COLOR_SUCCESS,
            marginTop: 6,
          }}
        >
          {message}
        </div>
      )}
    </section>
  );
}

function CallLogSection() {
  const [entries, setEntries] = useState<CallLogEntry[]>([]);
  const [expanded, setExpanded] = useState<number | null>(null);
  const [clearing, setClearing] = useState(false);

  const refresh = async () => {
    setEntries(await remoteCallLogList(20).catch(() => []));
  };

  useEffect(() => {
    void refresh();
  }, []);

  const clearLog = async () => {
    if (
      !confirm(
        "Soft-delete all call log entries? You can still restore them from Trash for 30 days.",
      )
    )
      return;
    setClearing(true);
    try {
      await remoteCallLogClear();
      await refresh();
    } finally {
      setClearing(false);
    }
  };

  return (
    <section style={{ marginTop: 16 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ margin: 0, fontSize: 15, color: "var(--ink)" }}>
          Call log ({entries.length})
        </h2>
        {entries.length > 0 && (
          <button onClick={clearLog} disabled={clearing} style={{ fontSize: 11 }}>
            {clearing ? "Clearing…" : "Clear log"}
          </button>
        )}
      </div>
      {entries.length === 0 && (
        <div style={{ fontSize: "var(--text-xs)", color: TEXT_MUTED, marginTop: 6 }}>
          No remote calls yet. Enable Claude for ledger review + run a review to see entries
          here.
        </div>
      )}
      <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 4 }}>
        {entries.map((e) => {
          const isExpanded = expanded === e.id;
          const outcome = e.error ? "error" : e.completed_at ? "ok" : "in-flight";
          const outcomeColor =
            outcome === "ok" ? COLOR_SUCCESS : outcome === "error" ? COLOR_DANGER : COLOR_AMBER;
          return (
            <div
              key={e.id}
              onClick={() => setExpanded(isExpanded ? null : e.id)}
              style={{ ...settingsListRow, cursor: "pointer" }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  fontSize: "var(--text-xs)",
                  color: "var(--ink)",
                }}
              >
                <span>
                  <span style={{ color: outcomeColor }}>●</span>{" "}
                  {new Date(e.started_at * 1000).toLocaleString()} · {e.skill} · {e.model}
                </span>
                <span style={{ color: TEXT_MUTED }}>
                  {e.redaction_count > 0 && `${e.redaction_count} redacted · `}
                  {e.cost_pence != null && `£${(e.cost_pence / 100).toFixed(2)}`}
                </span>
              </div>
              {isExpanded && (
                <div style={{ marginTop: 6, fontSize: 11, color: TEXT_SECONDARY }}>
                  <div>
                    <strong>Reason:</strong> {e.user_visible_reason}
                  </div>
                  <div style={{ marginTop: 4 }}>
                    <strong>Prompt (redacted, this is what left your Mac):</strong>
                    <pre style={{ ...settingsCodeBlock, marginTop: 4 }}>
                      {e.prompt_redacted}
                    </pre>
                  </div>
                  {e.response_text && (
                    <div style={{ marginTop: 4 }}>
                      <strong>Response:</strong>
                      <pre style={{ ...settingsCodeBlock, marginTop: 4 }}>
                        {e.response_text}
                      </pre>
                    </div>
                  )}
                  {e.error && (
                    <div style={{ color: COLOR_DANGER, marginTop: 4 }}>
                      <strong>Error:</strong> {e.error}
                    </div>
                  )}
                  {e.input_tokens != null && (
                    <div style={{ color: TEXT_MUTED, marginTop: 4 }}>
                      Tokens: {e.input_tokens} in / {e.output_tokens} out
                    </div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

export default function AiTab() {
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string>("");

  useEffect(() => {
    void (async () => {
      setLoading(true);
      const s = await ollamaStatus().catch(
        () => ({ reachable: false, models: [] }) as OllamaStatus,
      );
      setStatus(s);
      const stored = await settingGet(DEFAULT_MODEL_KEY);
      if (stored) {
        setSelected(stored);
      } else if (s.models.length > 0) {
        // Auto-persist the first installed model so the backend fallback (which
        // reads this setting) doesn't request a hardcoded tag that may not exist
        // on this system — the failure mode that broke older Ollama users.
        const first = s.models[0];
        await settingSet(DEFAULT_MODEL_KEY, first);
        setSelected(first);
      } else {
        setSelected("");
      }
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
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>
          Local brain (Ollama)
        </h2>
        {loading && <div style={{ fontSize: "var(--text-sm)", color: TEXT_MUTED }}>Checking…</div>}
        {!loading &&
          status &&
          (status.reachable ? (
            <div style={settingsStatusGood}>
              <div style={{ fontSize: "var(--text-sm)", color: COLOR_SUCCESS, fontWeight: 600 }}>
                ● Ready
                <span
                  style={{ color: TEXT_SECONDARY, fontWeight: 400, marginLeft: 6 }}
                >
                  — {status.models.length} model(s) installed
                </span>
              </div>
              <label
                style={{
                  display: "block",
                  marginTop: 8,
                  fontSize: "var(--text-sm)",
                  color: "var(--ink)",
                }}
              >
                Default model:
                <select
                  value={selected}
                  onChange={(e) => void onModelChange(e.target.value)}
                  style={{ marginLeft: 8 }}
                >
                  {status.models.length === 0 && <option value="">(no models)</option>}
                  {status.models.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          ) : (
            <div style={settingsStatusWarn}>
              <div style={{ fontSize: "var(--text-sm)", color: COLOR_AMBER, fontWeight: 600 }}>
                ● Unreachable
              </div>
              <div style={{ fontSize: "var(--text-xs)", color: TEXT_SECONDARY, marginTop: 4 }}>
                Start Ollama to enable chat, CalDAV AI review, and embeddings.{" "}
                <a
                  href="https://ollama.com/download"
                  target="_blank"
                  rel="noreferrer"
                  style={{ color: "var(--ink)" }}
                >
                  Install Ollama →
                </a>
              </div>
            </div>
          ))}
      </section>

      <EmbeddingsSection />

      <RemoteProvidersSection />

      <CallLogSection />

      <DeveloperSection />
    </div>
  );
}

function DeveloperSection() {
  const [sandboxEnabled, setSandboxEnabled] = useState(false);

  useEffect(() => {
    void (async () => {
      const v = await settingGet("bank_sandbox_enabled").catch(() => null);
      setSandboxEnabled(v === "true");
    })();
  }, []);

  const toggle = async (v: boolean) => {
    setSandboxEnabled(v);
    await settingSet("bank_sandbox_enabled", v ? "true" : "false");
  };

  return (
    <section>
      <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>Developer</h2>
      <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: "var(--text-sm)", color: "var(--ink)" }}>
        <input
          type="checkbox"
          checked={sandboxEnabled}
          onChange={(e) => void toggle(e.target.checked)}
        />
        Enable GoCardless sandbox institution
      </label>
      <p style={{ fontSize: "var(--text-xs)", color: TEXT_MUTED, marginTop: 4 }}>
        When on, the institution picker includes a SANDBOX test bank that returns
        deterministic fake transactions. For development only.
      </p>
    </section>
  );
}

function EmbeddingsSection() {
  const [status, setStatus] = useState<EmbeddingsStatus | null>(null);
  const [rebuilding, setRebuilding] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setStatus(await embeddingsStatus());
    } catch {
      setStatus(null);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const rebuild = async () => {
    if (!confirm("Rebuild will clear the embedding index and re-embed everything. Continue?"))
      return;
    setRebuilding(true);
    setMessage(null);
    try {
      const cleared = await embeddingsRebuild();
      setMessage(
        `Cleared ${cleared} vector(s). Re-indexing started — leave Manor open until it finishes.`,
      );
      await refresh();
    } catch (e) {
      setMessage(`Rebuild failed: ${e}`);
    } finally {
      setRebuilding(false);
    }
  };

  return (
    <section>
      <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>Embeddings</h2>
      {!status && <div style={{ fontSize: "var(--text-xs)", color: TEXT_MUTED }}>Loading…</div>}
      {status && (
        <>
          <div style={{ fontSize: "var(--text-sm)", color: "var(--ink)" }}>
            <span style={{ color: TEXT_MUTED }}>Model:</span> {status.model}
          </div>
          <div style={{ fontSize: "var(--text-sm)", marginTop: 4, color: "var(--ink)" }}>
            <span style={{ color: TEXT_MUTED }}>Indexed:</span> {status.total} vector(s)
            {status.by_entity_type.length > 0 && " ("}
            {status.by_entity_type.map(([t, n], i) => (
              <span key={t}>
                {i > 0 && ", "}
                {t}: {n}
              </span>
            ))}
            {status.by_entity_type.length > 0 && ")"}
          </div>
          <button
            onClick={rebuild}
            disabled={rebuilding}
            style={{ marginTop: 8, fontSize: 12 }}
          >
            {rebuilding ? "Rebuilding…" : "Rebuild embeddings"}
          </button>
          {message && (
            <div
              style={{
                fontSize: 11,
                color: message.includes("failed") ? COLOR_DANGER : COLOR_SUCCESS,
                marginTop: 6,
              }}
            >
              {message}
            </div>
          )}
        </>
      )}
    </section>
  );
}
