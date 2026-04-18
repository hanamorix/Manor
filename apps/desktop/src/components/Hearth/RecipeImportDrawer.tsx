import { useState } from "react";
import * as ipc from "../../lib/recipe/recipe-ipc";
import type { ImportPreview } from "../../lib/recipe/recipe-ipc";
import { RecipeEditDrawer } from "./RecipeEditDrawer";

interface Props {
  onClose: () => void;
  onSaved: (id: string) => void;
}

export function RecipeImportDrawer({ onClose, onSaved }: Props) {
  const [url, setUrl] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [fetching, setFetching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchPreview = async () => {
    setFetching(true);
    setError(null);
    try {
      const p = await ipc.importPreview(url);
      setPreview(p);
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
    } finally {
      setFetching(false);
    }
  };

  if (preview) {
    // Delegate preview UI + edit to RecipeEditDrawer, overriding save to use
    // importCommit so the hero image gets linked.
    // Note: nothing to cancel on Close here — hero image is staged only at
    // commit (recipe_import_commit), never during preview. The 24h orphan sweep
    // in lib.rs handles any crash-orphaned staged attachments on app restart.
    return (
      <RecipeEditDrawer
        initialDraft={preview.recipe_draft}
        title="Confirm recipe"
        saveLabel="Save to library"
        onClose={onClose}
        onSaved={onSaved}
        onSubmit={async (draft) => {
          return await ipc.importCommit(draft, preview.hero_image_url);
        }}
      />
    );
  }

  return (
    <div style={{
      position: "fixed",
      top: 0,
      right: 0,
      bottom: 0,
      width: 480,
      background: "var(--paper, #fff)",
      borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24,
      overflow: "auto",
      zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20 }}>Import from URL</h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>URL</label>
      <input
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && url && !fetching) void fetchPreview(); }}
        placeholder="https://…"
        style={{ width: "100%", marginBottom: 12, padding: 6, boxSizing: "border-box" }}
      />
      <button
        type="button"
        onClick={() => void fetchPreview()}
        disabled={fetching || !url}
      >
        {fetching ? "Fetching…" : "Fetch"}
      </button>

      {error && (
        <div style={{ color: "var(--ink-danger, #b00020)", marginTop: 12 }}>
          {error}
        </div>
      )}
    </div>
  );
}
