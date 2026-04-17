import { useEffect, useState } from "react";
import { Check, ArrowRight, RefreshCw } from "lucide-react";
import * as ipc from "../../lib/ledger/bank-ipc";
import { Button } from "../../lib/ui";

type Mode =
  | { kind: "connect" }
  | { kind: "reconnect"; account_id: number };

type Stage =
  | { kind: "loading" }
  | { kind: "byok" }
  | { kind: "pick"; country: string; institutions: ipc.UiInstitution[]; search: string }
  | {
      kind: "authorizing";
      institution: ipc.UiInstitution;
      reference: string;
      requisition_id: string;
      granted: number;
    }
  | { kind: "syncing"; account_ids: number[] }
  | { kind: "error"; message: string };

interface Props {
  mode: Mode;
  onClose: () => void;
}

export function ConnectBankDrawer({ mode, onClose }: Props) {
  const [stage, setStage] = useState<Stage>({ kind: "loading" });
  const replacesAccountId = mode.kind === "reconnect" ? mode.account_id : null;

  // If the user closes the drawer while the loopback server is still
  // listening (authorizing stage), tell the Rust side to release the port
  // immediately rather than leaking it for ~10 minutes.
  useEffect(() => {
    if (stage.kind !== "authorizing") return;
    const reference = stage.reference;
    return () => {
      ipc.cancelConnect(reference).catch(() => {});
    };
  }, [stage]);

  useEffect(() => {
    (async () => {
      try {
        const hasCreds = await ipc.credentialsStatus();
        if (!hasCreds) {
          setStage({ kind: "byok" });
        } else {
          await loadInstitutions("GB");
        }
      } catch (e: unknown) {
        setStage({ kind: "error", message: e instanceof Error ? e.message : String(e) });
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function loadInstitutions(country: string) {
    const institutions = await ipc.listInstitutions(country);
    setStage({ kind: "pick", country, institutions, search: "" });
  }

  async function saveCredsAndContinue(secret_id: string, secret_key: string) {
    try {
      await ipc.saveCredentials(secret_id, secret_key);
      await loadInstitutions("GB");
    } catch (e: unknown) {
      setStage({ kind: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }

  async function pickInstitution(inst: ipc.UiInstitution) {
    try {
      const begin = await ipc.beginConnect(inst.id);
      setStage({
        kind: "authorizing",
        institution: inst,
        reference: begin.reference,
        requisition_id: begin.requisition_id,
        granted: begin.max_historical_days_granted,
      });
      await ipc.openAuthUrl(begin.auth_url);

      const resp = await ipc.completeConnect({
        reference: begin.reference,
        requisition_id: begin.requisition_id,
        institution_id: inst.id,
        institution_name: inst.name,
        institution_logo_url: inst.logo_url,
        max_historical_days_granted: begin.max_historical_days_granted,
        replaces_account_id: replacesAccountId,
      });
      setStage({ kind: "syncing", account_ids: resp.account_ids });
    } catch (e: unknown) {
      setStage({ kind: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }

  return (
    <div
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--scrim)",
        display: "flex",
        justifyContent: "flex-end",
        zIndex: 1000,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 480,
          background: "var(--paper)",
          padding: 24,
          color: "var(--ink)",
          overflowY: "auto",
        }}
      >
        <h2 style={{ marginTop: 0 }}>Connect a bank</h2>

        {stage.kind === "loading" && <p>Loading…</p>}

        {stage.kind === "byok" && (
          <ByokForm onSubmit={saveCredsAndContinue} onCancel={onClose} />
        )}

        {stage.kind === "pick" && (
          <PickForm
            country={stage.country}
            search={stage.search}
            institutions={stage.institutions}
            onCountry={async (c) => {
              setStage({ kind: "loading" });
              try {
                await loadInstitutions(c);
              } catch (e: unknown) {
                setStage({
                  kind: "error",
                  message: e instanceof Error ? e.message : String(e),
                });
              }
            }}
            onSearch={(s) =>
              setStage((prev) =>
                prev.kind === "pick" ? { ...prev, search: s } : prev
              )
            }
            onPick={pickInstitution}
          />
        )}

        {stage.kind === "authorizing" && (
          <div>
            <p>Waiting for {stage.institution.name} authorisation…</p>
            <p style={{ color: "var(--ink-soft)", fontSize: 13 }}>
              Complete the login in your browser. Manor will take over automatically.
            </p>
          </div>
        )}

        {stage.kind === "syncing" && (
          <div>
            <h3>
              ✓ Connected {stage.account_ids.length} account
              {stage.account_ids.length === 1 ? "" : "s"}
            </h3>
            <p>Syncing 180 days of transactions — this may take up to 30 seconds.</p>
            <Button variant="primary" icon={Check} onClick={onClose}>Done</Button>
          </div>
        )}

        {stage.kind === "error" && (
          <div>
            <h3>Something went wrong</h3>
            <pre
              style={{
                whiteSpace: "pre-wrap",
                background: "var(--hairline)",
                padding: 12,
                borderRadius: 4,
              }}
            >
              {stage.message}
            </pre>
            <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
              <Button
                variant="primary"
                icon={RefreshCw}
                onClick={() => {
                  setStage({ kind: "loading" });
                  (async () => {
                    try {
                      const hasCreds = await ipc.credentialsStatus();
                      if (!hasCreds) {
                        setStage({ kind: "byok" });
                      } else {
                        await loadInstitutions("GB");
                      }
                    } catch (e: unknown) {
                      setStage({
                        kind: "error",
                        message: e instanceof Error ? e.message : String(e),
                      });
                    }
                  })();
                }}
              >
                Try again
              </Button>
              <Button variant="secondary" onClick={onClose}>Close</Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function ByokForm({
  onSubmit,
  onCancel,
}: {
  onSubmit: (id: string, key: string) => void;
  onCancel: () => void;
}) {
  const [id, setId] = useState("");
  const [key, setKey] = useState("");
  return (
    <div>
      <p>
        Manor connects to your bank through <b>GoCardless</b>, a free EU/UK service.
        You'll need a GoCardless account and API keys. Takes about 3 minutes, one time.
      </p>
      <ol style={{ color: "var(--ink-soft)", fontSize: 14 }}>
        <li>
          <button
            style={{
              background: "none",
              border: "none",
              color: "var(--ink)",
              padding: 0,
              cursor: "pointer",
            }}
            onClick={() => ipc.openAuthUrl("https://bankaccountdata.gocardless.com/")}
          >
            Create a free account ↗
          </button>
        </li>
        <li>Go to User Secrets → copy your Secret ID and Secret Key.</li>
        <li>Paste them below.</li>
      </ol>
      <label style={{ display: "block", marginTop: 12 }}>
        Secret ID
        <input
          type="text"
          value={id}
          onChange={(e) => setId(e.target.value)}
          style={{
            width: "100%",
            padding: 8,
            marginTop: 4,
            background: "var(--paper)",
            color: "var(--ink)",
            border: "1px solid var(--hairline-strong)",
          }}
        />
      </label>
      <label style={{ display: "block", marginTop: 12 }}>
        Secret Key
        <input
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          style={{
            width: "100%",
            padding: 8,
            marginTop: 4,
            background: "var(--paper)",
            color: "var(--ink)",
            border: "1px solid var(--hairline-strong)",
          }}
        />
      </label>
      <p style={{ color: "var(--ink-soft)", fontSize: "var(--text-xs)", marginTop: 12 }}>
        Your keys are stored in macOS Keychain. They never leave this device.
      </p>
      <div
        style={{
          display: "flex",
          gap: 8,
          justifyContent: "flex-end",
          marginTop: 20,
        }}
      >
        <Button variant="secondary" onClick={onCancel}>Cancel</Button>
        <Button
          variant="primary"
          icon={ArrowRight}
          onClick={() => onSubmit(id.trim(), key.trim())}
          disabled={!id.trim() || !key.trim()}
        >
          Continue
        </Button>
      </div>
    </div>
  );
}

function PickForm({
  country,
  search,
  institutions,
  onCountry,
  onSearch,
  onPick,
}: {
  country: string;
  search: string;
  institutions: ipc.UiInstitution[];
  onCountry: (c: string) => void;
  onSearch: (s: string) => void;
  onPick: (i: ipc.UiInstitution) => void;
}) {
  const filtered = institutions.filter((i) =>
    i.name.toLowerCase().includes(search.toLowerCase())
  );
  const countries: [string, string][] = [
    ["GB", "United Kingdom"],
    ["IE", "Ireland"],
    ["FR", "France"],
    ["DE", "Germany"],
    ["ES", "Spain"],
    ["IT", "Italy"],
    ["NL", "Netherlands"],
  ];
  return (
    <div>
      <label style={{ display: "block", marginBottom: 12 }}>
        Country
        <select
          value={country}
          onChange={(e) => onCountry(e.target.value)}
          style={{ marginLeft: 12, padding: 6 }}
        >
          {countries.map(([c, n]) => (
            <option key={c} value={c}>
              {n}
            </option>
          ))}
        </select>
      </label>
      <input
        type="text"
        placeholder="Type to filter institutions…"
        value={search}
        onChange={(e) => onSearch(e.target.value)}
        style={{
          width: "100%",
          padding: 8,
          marginBottom: 12,
          background: "var(--paper)",
          color: "var(--ink)",
          border: "1px solid var(--hairline-strong)",
        }}
      />
      <div style={{ maxHeight: 400, overflowY: "auto" }}>
        {filtered.map((i) => (
          <button
            key={i.id}
            onClick={() => onPick(i)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 12,
              width: "100%",
              background: "none",
              border: "none",
              padding: "10px 12px",
              color: "var(--ink)",
              textAlign: "left",
              cursor: "pointer",
              borderBottom: "1px solid var(--hairline-strong)",
            }}
          >
            {i.logo_url && <img src={i.logo_url} width={24} height={24} alt="" />}
            <span>{i.name}</span>
            {i.is_sandbox && (
              <span
                style={{
                  marginLeft: "auto",
                  background: "var(--hairline-strong)",
                  color: "var(--ink)",
                  padding: "2px 6px",
                  borderRadius: 3,
                  fontSize: 10,
                }}
              >
                SANDBOX
              </span>
            )}
          </button>
        ))}
      </div>
    </div>
  );
}
