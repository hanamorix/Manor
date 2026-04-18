import { useEffect, useState, useCallback } from "react";
import { FileText, Image as ImageIcon, File, Plus } from "lucide-react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import * as ipc from "../../lib/asset/ipc";
import type { AttachmentSummary } from "../../lib/asset/ipc";

interface Props {
  assetId: string;
}

function iconFor(mime: string) {
  if (mime.includes("pdf")) return FileText;
  if (mime.startsWith("image/")) return ImageIcon;
  return File;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleDateString(undefined, {
    month: "short", day: "numeric", year: "numeric",
  });
}

export function DocumentList({ assetId }: Props) {
  const [docs, setDocs] = useState<AttachmentSummary[]>([]);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    void ipc.listDocuments(assetId).then(setDocs).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e));
    });
  }, [assetId]);

  useEffect(() => { reload(); }, [reload]);

  const addDoc = async () => {
    const picked = await openFileDialog({ multiple: false, directory: false });
    const path = typeof picked === "string" ? picked : null;
    if (!path) return;
    try {
      await ipc.attachDocumentFromPath(assetId, path);
      reload();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const openDoc = async (uuid: string) => {
    try {
      const absPath = await invoke<string>("attachment_get_path_by_uuid", { uuid });
      await openPath(absPath);
    } catch (e: unknown) {
      setError(`Couldn't open — ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  return (
    <div>
      {error && <div style={{ color: "var(--ink-danger, #b00020)", marginBottom: 8 }}>{error}</div>}
      {docs.map((d) => {
        const Icon = iconFor(d.mime_type);
        return (
          <div key={d.id}
               onClick={() => void openDoc(d.uuid)}
               style={{
                 display: "flex", alignItems: "center", gap: 12,
                 padding: "8px 12px",
                 borderBottom: "1px solid var(--hairline, #e5e5e5)",
                 cursor: "pointer",
               }}>
            <Icon size={16} strokeWidth={1.8} color="var(--ink-soft, #999)" />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 14, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                {d.original_name}
              </div>
              <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>
                {formatSize(d.size_bytes)} · {formatDate(d.created_at)}
              </div>
            </div>
          </div>
        );
      })}
      <button type="button" onClick={addDoc}
        style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 4 }}>
        <Plus size={14} strokeWidth={1.8} /> Add document
      </button>
    </div>
  );
}
