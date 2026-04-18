import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, Pencil, Trash2, ImageOff } from "lucide-react";
import * as ipc from "../../lib/asset/ipc";
import type { Asset } from "../../lib/asset/ipc";
import { AssetEditDrawer } from "./AssetEditDrawer";
import { DocumentList } from "./DocumentList";

interface Props { id: string; onBack: () => void }

const CATEGORY_LABEL: Record<string, string> = {
  appliance: "Appliance",
  vehicle: "Vehicle",
  fixture: "Fixture",
  other: "Other",
};

export function AssetDetail({ id, onBack }: Props) {
  const [asset, setAsset] = useState<Asset | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  const reload = useCallback(() => {
    setLoaded(false);
    void ipc.get(id).then((a) => { setAsset(a); setLoaded(true); });
  }, [id]);

  useEffect(() => { reload(); }, [reload]);

  useEffect(() => {
    const uuid = asset?.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void ipc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [asset?.hero_attachment_uuid]);

  if (!loaded) return <div style={{ padding: 32 }}>Loading…</div>;
  if (!asset) {
    return (
      <div style={{ padding: 32, maxWidth: 720, margin: "0 auto" }}>
        <button type="button" onClick={onBack}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <ArrowLeft size={14} strokeWidth={1.8} /> Back
        </button>
        <h1 style={{ fontSize: 24, fontWeight: 600, marginTop: 16 }}>Asset not found</h1>
        <p style={{ color: "var(--ink-soft, #999)" }}>
          It may have been moved to Trash. You can restore it from the Trash view.
        </p>
      </div>
    );
  }

  const handleDelete = async () => {
    if (!window.confirm("Move this asset to Trash?")) return;
    try {
      await ipc.deleteAsset(id);
      onBack();
    } catch (e: unknown) {
      window.alert(`Failed to delete: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const meta1 = [
    CATEGORY_LABEL[asset.category] ?? asset.category,
    asset.make,
    asset.model,
  ].filter(Boolean).join(" · ");

  const meta2 = [
    asset.serial_number ? `Serial: ${asset.serial_number}` : null,
    asset.purchase_date ? `Purchased: ${new Date(asset.purchase_date + "T00:00:00").toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" })}` : null,
  ].filter(Boolean).join("  ·  ");

  return (
    <div style={{ padding: 32, maxWidth: 720, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <button type="button" onClick={onBack}
          style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <ArrowLeft size={14} strokeWidth={1.8} /> Back
        </button>
        <div style={{ display: "flex", gap: 8 }}>
          <button type="button" onClick={() => setEditing(true)}>
            <Pencil size={14} strokeWidth={1.8} /> Edit
          </button>
          <button type="button" onClick={handleDelete}>
            <Trash2 size={14} strokeWidth={1.8} /> Delete
          </button>
        </div>
      </div>

      <div style={{
        aspectRatio: "16 / 9",
        maxHeight: 360,
        background: "var(--paper-muted, #f5f5f5)",
        display: "flex", alignItems: "center", justifyContent: "center",
        marginBottom: 16, borderRadius: 6, overflow: "hidden",
      }}>
        {heroSrc ? (
          <img src={heroSrc} alt={asset.name}
            style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <ImageOff size={48} strokeWidth={1.4} color="var(--ink-soft, #999)" />
        )}
      </div>

      <h1 style={{ fontSize: 28, fontWeight: 600, margin: 0 }}>{asset.name}</h1>
      {meta1 && <div style={{ color: "var(--ink-soft, #999)", marginTop: 4 }}>{meta1}</div>}
      {meta2 && <div style={{ color: "var(--ink-soft, #999)", marginTop: 2 }}>{meta2}</div>}

      {asset.notes.trim() && (
        <>
          <h2 style={{ marginTop: 32, fontSize: 18 }}>Notes</h2>
          <pre style={{
            whiteSpace: "pre-wrap", fontFamily: "inherit",
            background: "var(--paper-muted, #f5f5f5)",
            padding: 16, borderRadius: 6,
          }}>
            {asset.notes}
          </pre>
        </>
      )}

      <h2 style={{ marginTop: 32, fontSize: 18 }}>Documents</h2>
      <DocumentList assetId={id} />

      {editing && (
        <AssetEditDrawer
          assetId={id}
          onClose={() => setEditing(false)}
          onSaved={() => reload()}
        />
      )}
    </div>
  );
}
