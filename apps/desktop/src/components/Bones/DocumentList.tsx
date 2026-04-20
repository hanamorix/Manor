import { useEffect, useState, useCallback } from "react";
import { FileText, Image as ImageIcon, File, Plus, Sparkles } from "lucide-react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import * as ipc from "../../lib/asset/ipc";
import type { AttachmentSummary } from "../../lib/asset/ipc";
import { usePdfExtractStore } from "../../lib/pdf_extract/state";

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
  const {
    pendingByAttachment,
    loadPendingFlag,
    extractOllama,
    extractClaude,
    extractStatus,
    lastExtractMessage,
    clearLastMessage,
  } = usePdfExtractStore();

  const reload = useCallback(() => {
    void ipc.listDocuments(assetId).then((rows) => {
      setDocs(rows);
      for (const d of rows) {
        if (d.mime_type === "application/pdf") {
          void loadPendingFlag(d.uuid);
        }
      }
    }).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e));
    });
  }, [assetId, loadPendingFlag]);

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

  const isExtracting = extractStatus.kind === "extracting";

  return (
    <div>
      {error && <div style={{ color: "var(--ink-danger, #b00020)", marginBottom: 8 }}>{error}</div>}
      {extractStatus.kind === "error" && (
        <div style={{
          border: "1px solid var(--ink-danger, #b00020)",
          background: "var(--danger-bg, #fff5f5)",
          color: "var(--ink-danger, #b00020)",
          padding: 8,
          borderRadius: 4,
          marginBottom: 8,
          fontSize: 13,
        }}>
          {extractStatus.message}
        </div>
      )}
      {lastExtractMessage && (
        <div
          onClick={() => clearLastMessage()}
          style={{
            border: "1px solid var(--hairline, #e5e5e5)",
            background: "var(--surface-subtle, #f4f6fa)",
            padding: 8,
            borderRadius: 4,
            marginBottom: 8,
            fontSize: 13,
            cursor: "pointer",
          }}
        >
          {lastExtractMessage} <span style={{ color: "var(--ink-soft, #999)" }}>(dismiss)</span>
        </div>
      )}
      {docs.map((d) => {
        const Icon = iconFor(d.mime_type);
        const isPdf = d.mime_type === "application/pdf";
        const hasPending = pendingByAttachment[d.uuid] === true;
        return (
          <div key={d.id}
               style={{
                 padding: "8px 12px",
                 borderBottom: "1px solid var(--hairline, #e5e5e5)",
               }}>
            <div
              onClick={() => void openDoc(d.uuid)}
              style={{
                display: "flex", alignItems: "center", gap: 12,
                cursor: "pointer",
              }}
            >
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
            {isPdf && (
              <div style={{ display: "flex", gap: 8, marginTop: 6, marginLeft: 28 }}>
                <button
                  type="button"
                  onClick={() => void extractOllama(d.uuid, assetId)}
                  disabled={isExtracting}
                  style={{ fontSize: 12 }}
                >
                  {isExtracting && extractStatus.tier === "ollama"
                    ? "Extracting…"
                    : "Extract maintenance schedules"}
                </button>
                {hasPending && (
                  <button
                    type="button"
                    onClick={() => void extractClaude(d.uuid, assetId)}
                    disabled={isExtracting}
                    style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 4 }}
                  >
                    <Sparkles size={12} strokeWidth={1.8} />
                    {isExtracting && extractStatus.tier === "claude"
                      ? "Extracting with Claude…"
                      : "Re-extract with Claude"}
                  </button>
                )}
              </div>
            )}
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
