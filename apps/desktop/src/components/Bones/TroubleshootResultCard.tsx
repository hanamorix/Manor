import { X, Sparkles } from "lucide-react";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import type { PipelineOutcome } from "../../lib/repair/ipc";
import { useRepairStore } from "../../lib/repair/state";
import { RepairMarkdown } from "./RepairMarkdown";

interface Props {
  assetId: string;
  outcome: PipelineOutcome;
}

export function TroubleshootResultCard({ assetId, outcome }: Props) {
  const { searchClaude, clearLastOutcome, searchStatus, lastSymptomByAsset } = useRepairStore();
  const symptom = lastSymptomByAsset[assetId] ?? outcome.note?.symptom ?? "";

  const onTryClaude = async () => {
    try {
      await searchClaude(assetId, symptom);
    } catch {
      // error rendered via searchStatus
    }
  };

  const borderColor = outcome.empty_or_failed
    ? "var(--warn-border, #d4a72c)"
    : "var(--border, #e5e5e5)";

  // Mode A: success with persisted note.
  if (outcome.note && !outcome.empty_or_failed) {
    const note = outcome.note;
    return (
      <div style={{
        border: `1px solid ${borderColor}`,
        borderRadius: 6,
        padding: 16,
        marginBottom: 16,
        background: "var(--surface-elevated, #fafafa)",
      }}>
        <div style={{ display: "flex", alignItems: "center", marginBottom: 8 }}>
          <strong style={{ flex: 1 }}>{note.symptom}</strong>
          <span style={{
            fontSize: 11,
            padding: "2px 6px",
            borderRadius: 4,
            background: note.tier === "claude" ? "var(--accent-bg, #eef5ff)" : "var(--surface-subtle, #f4f4f4)",
            color: "var(--ink-soft, #666)",
            marginRight: 8,
          }}>
            {note.tier}
          </span>
          <button
            type="button"
            onClick={() => clearLastOutcome(assetId)}
            aria-label="Close result"
            style={{ background: "none", border: "none", cursor: "pointer" }}
          >
            <X size={14} />
          </button>
        </div>
        <RepairMarkdown body={note.body_md} />
        {note.sources.length > 0 && (
          <div style={{ marginTop: 12 }}>
            <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Sources</div>
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {note.sources.map((s) => (
                <li key={s.url}>
                  <a href={s.url} onClick={(e) => { e.preventDefault(); void openUrl(s.url); }} style={{ cursor: "pointer" }}>
                    {s.title}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        )}
        {note.video_sources && note.video_sources.length > 0 && (
          <div style={{ marginTop: 8 }}>
            <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>Videos</div>
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {note.video_sources.map((s) => (
                <li key={s.url}>
                  <a href={s.url} onClick={(e) => { e.preventDefault(); void openUrl(s.url); }} style={{ cursor: "pointer" }}>
                    {s.title}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        )}
        {note.tier === "ollama" && (
          <div style={{ marginTop: 12 }}>
            <button
              type="button"
              onClick={onTryClaude}
              disabled={searchStatus.kind === "searching"}
            >
              <Sparkles size={14} /> Try with Claude
            </button>
          </div>
        )}
      </div>
    );
  }

  // Mode B: empty/failed Ollama — sources present, no body, offer Claude.
  return (
    <div style={{
      border: `1px solid ${borderColor}`,
      borderRadius: 6,
      padding: 16,
      marginBottom: 16,
      background: "var(--surface-elevated, #fafafa)",
    }}>
      <div style={{ display: "flex", alignItems: "center", marginBottom: 8 }}>
        <strong style={{ flex: 1 }}>
          The local model didn't return a usable answer for "{symptom}".
        </strong>
        <button
          type="button"
          onClick={() => clearLastOutcome(assetId)}
          aria-label="Dismiss"
          style={{ background: "none", border: "none", cursor: "pointer" }}
        >
          <X size={14} />
        </button>
      </div>
      {outcome.sources.length > 0 && (
        <div style={{ marginBottom: 12 }}>
          <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", marginBottom: 4 }}>
            Sources we found (unread):
          </div>
          <ul style={{ margin: 0, paddingLeft: 18 }}>
            {outcome.sources.map((s) => (
              <li key={s.url}>
                <a href={s.url} onClick={(e) => { e.preventDefault(); void openUrl(s.url); }} style={{ cursor: "pointer" }}>
                  {s.title}
                </a>
              </li>
            ))}
          </ul>
        </div>
      )}
      <button
        type="button"
        onClick={onTryClaude}
        disabled={searchStatus.kind === "searching"}
      >
        <Sparkles size={14} /> Try with Claude
      </button>
    </div>
  );
}
