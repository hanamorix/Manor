import { useEffect, useState } from "react";
import { useRepairStore } from "../../lib/repair/state";
import { RepairNoteCard } from "./RepairNoteCard";
import { TroubleshootResultCard } from "./TroubleshootResultCard";

interface Props {
  assetId: string;
}

const MAX_SYMPTOM_LEN = 200;

export function TroubleshootBlock({ assetId }: Props) {
  const {
    notesByAsset,
    lastOutcomeByAsset,
    searchStatus,
    loadForAsset,
    searchOllama,
  } = useRepairStore();
  const [symptom, setSymptom] = useState("");

  const outcome = lastOutcomeByAsset[assetId] ?? null;
  const notes = notesByAsset[assetId] ?? [];
  const historyNotes = outcome?.note
    ? notes.filter((n) => n.id !== outcome.note!.id)
    : notes;

  useEffect(() => {
    if (!notesByAsset[assetId]) void loadForAsset(assetId);
  }, [assetId, notesByAsset, loadForAsset]);

  const disabled = searchStatus.kind === "searching" || symptom.trim().length === 0;

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (disabled) return;
    try {
      await searchOllama(assetId, symptom.trim());
      setSymptom("");
    } catch {
      // error surfaces via searchStatus
    }
  };

  return (
    <section style={{ marginTop: 24 }}>
      <div style={{ marginBottom: 12 }}>
        <h3 style={{ margin: 0 }}>Troubleshoot</h3>
        <div style={{ fontSize: 12, color: "var(--ink-soft, #888)", fontStyle: "italic" }}>
          Search the web and summarise — uses your local model first.
        </div>
      </div>

      {searchStatus.kind === "error" && (
        <div style={{
          border: "1px solid var(--danger, #c43)",
          background: "var(--danger-bg, #fff5f5)",
          color: "var(--danger, #c43)",
          padding: 8,
          borderRadius: 4,
          marginBottom: 8,
          fontSize: 13,
        }}>
          {searchStatus.message}
        </div>
      )}

      <form
        onSubmit={onSubmit}
        style={{ display: "flex", gap: 8, marginBottom: 12 }}
      >
        <input
          type="text"
          value={symptom}
          onChange={(e) => setSymptom(e.target.value)}
          maxLength={MAX_SYMPTOM_LEN}
          placeholder="What's wrong? e.g., won't drain, making grinding noise"
          style={{
            flex: 1,
            padding: 8,
            border: "1px solid var(--border, #e5e5e5)",
            borderRadius: 4,
          }}
        />
        <button type="submit" disabled={disabled}>
          {searchStatus.kind === "searching"
            ? searchStatus.tier === "claude"
              ? "Asking Claude…"
              : "Asking qwen2.5…"
            : "Search"}
        </button>
      </form>

      {outcome && <TroubleshootResultCard assetId={assetId} outcome={outcome} />}

      {historyNotes.length > 0 && (
        <div>
          {historyNotes.map((n) => <RepairNoteCard key={n.id} note={n} />)}
        </div>
      )}
    </section>
  );
}
