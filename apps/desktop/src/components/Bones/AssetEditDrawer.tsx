import { useEffect, useState } from "react";
import { ImageOff } from "lucide-react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import * as ipc from "../../lib/asset/ipc";
import type { Asset, AssetCategory, AssetDraft } from "../../lib/asset/ipc";

interface Props {
  assetId?: string;                       // undefined = create mode
  onClose: () => void;
  onSaved?: (id: string) => void;
}

const EMPTY_DRAFT: AssetDraft = {
  name: "",
  category: "appliance",
  make: null,
  model: null,
  serial_number: null,
  purchase_date: null,
  notes: "",
  hero_attachment_uuid: null,
};

const CATEGORIES: { key: AssetCategory; label: string }[] = [
  { key: "appliance", label: "Appliance" },
  { key: "vehicle",   label: "Vehicle" },
  { key: "fixture",   label: "Fixture" },
  { key: "other",     label: "Other" },
];

export function AssetEditDrawer({ assetId, onClose, onSaved }: Props) {
  const [draft, setDraft] = useState<AssetDraft>(EMPTY_DRAFT);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (assetId) {
      void ipc.get(assetId).then((a: Asset | null) => {
        if (a) setDraft({
          name: a.name, category: a.category,
          make: a.make, model: a.model, serial_number: a.serial_number,
          purchase_date: a.purchase_date, notes: a.notes,
          hero_attachment_uuid: a.hero_attachment_uuid,
        });
      });
    }
  }, [assetId]);

  useEffect(() => {
    const uuid = draft.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void ipc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [draft.hero_attachment_uuid]);

  const pickHero = async () => {
    if (!assetId) {
      setError("Save the asset once before adding a hero image");
      return;
    }
    const picked = await openFileDialog({ multiple: false, directory: false });
    const path = typeof picked === "string" ? picked : null;
    if (!path) return;
    try {
      const uuid = await ipc.attachHeroFromPath(assetId, path);
      setDraft({ ...draft, hero_attachment_uuid: uuid });
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const removeHero = async () => {
    if (!assetId) {
      // Create mode: nothing to persist yet; just clear local state.
      setDraft({ ...draft, hero_attachment_uuid: null });
      return;
    }
    try {
      // Persist immediately — symmetric with pickHero.
      const nextDraft = { ...draft, hero_attachment_uuid: null };
      await ipc.update(assetId, nextDraft);
      setDraft(nextDraft);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const save = async () => {
    if (!draft.name.trim()) { setError("Name required"); return; }
    setSaving(true); setError(null);
    try {
      const id = assetId
        ? (await ipc.update(assetId, draft), assetId)
        : await ipc.create(draft);
      onSaved?.(id);
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, width: 480,
      background: "var(--paper, #fff)", borderLeft: "1px solid var(--hairline, #e5e5e5)",
      padding: 24, overflow: "auto", zIndex: 50,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20 }}>{assetId ? "Edit asset" : "New asset"}</h2>
        <button type="button" onClick={onClose} aria-label="Close">✕</button>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Name</label>
      <input value={draft.name}
        onChange={(e) => setDraft({ ...draft, name: e.target.value })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }} />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Category</label>
      <select value={draft.category}
        onChange={(e) => setDraft({ ...draft, category: e.target.value as AssetCategory })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }}>
        {CATEGORIES.map((c) => <option key={c.key} value={c.key}>{c.label}</option>)}
      </select>

      <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
        <div style={{ flex: 1 }}>
          <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Make</label>
          <input value={draft.make ?? ""}
            onChange={(e) => setDraft({ ...draft, make: e.target.value || null })}
            style={{ width: "100%", padding: 6 }} />
        </div>
        <div style={{ flex: 1 }}>
          <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Model</label>
          <input value={draft.model ?? ""}
            onChange={(e) => setDraft({ ...draft, model: e.target.value || null })}
            style={{ width: "100%", padding: 6 }} />
        </div>
      </div>

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Serial number</label>
      <input value={draft.serial_number ?? ""}
        onChange={(e) => setDraft({ ...draft, serial_number: e.target.value || null })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }} />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Purchase date</label>
      <input type="date" value={draft.purchase_date ?? ""}
        onChange={(e) => setDraft({ ...draft, purchase_date: e.target.value || null })}
        style={{ width: "100%", marginBottom: 12, padding: 6 }} />

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Hero image</label>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
        <div style={{ width: 80, height: 60, background: "var(--paper-muted, #f5f5f5)",
                      display: "flex", alignItems: "center", justifyContent: "center",
                      borderRadius: 4, overflow: "hidden" }}>
          {heroSrc ? (
            <img src={heroSrc} alt="" style={{ width: "100%", height: "100%", objectFit: "cover" }} />
          ) : (
            <ImageOff size={20} strokeWidth={1.4} color="var(--ink-soft, #999)" />
          )}
        </div>
        <button type="button" onClick={pickHero} disabled={!assetId}>
          {draft.hero_attachment_uuid ? "Replace" : "Choose…"}
        </button>
        {draft.hero_attachment_uuid && (
          <button type="button" onClick={removeHero}>Remove</button>
        )}
      </div>
      {!assetId && (
        <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginBottom: 12 }}>
          Save first, then add a hero image.
        </div>
      )}

      <label style={{ display: "block", fontSize: 12, marginBottom: 4 }}>Notes (markdown)</label>
      <textarea value={draft.notes}
        onChange={(e) => setDraft({ ...draft, notes: e.target.value })}
        rows={6} style={{ width: "100%", fontFamily: "inherit", padding: 6 }} />

      {error && <div style={{ color: "var(--ink-danger, #b00020)", marginTop: 8 }}>{error}</div>}

      <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
        <button type="button" onClick={onClose}>Cancel</button>
        <button type="button" onClick={save} disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
      </div>
    </div>
  );
}
