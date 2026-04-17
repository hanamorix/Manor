import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Tag { id: number; name: string; color: string; created_at: number; }

async function loadSampleTag(): Promise<Tag | null> {
  const tags = await invoke<Tag[]>("tag_list").catch(() => [] as Tag[]);
  return tags.find((t) => t.name.toLowerCase() === "sample") ?? null;
}

async function loadSampleEntities(tagId: number): Promise<Array<[string, number]>> {
  return invoke<Array<[string, number]>>("entities_with_tag", { tagId }).catch(() => []);
}

export default function SampleDataBanner() {
  const [tag, setTag] = useState<Tag | null>(null);
  const [count, setCount] = useState<number>(0);
  const [deleting, setDeleting] = useState(false);

  const refresh = async () => {
    const t = await loadSampleTag();
    setTag(t);
    if (t) {
      const entities = await loadSampleEntities(t.id);
      setCount(entities.length);
    } else {
      setCount(0);
    }
  };

  useEffect(() => { void refresh(); }, []);

  const deleteAll = async () => {
    if (!tag) return;
    setDeleting(true);
    try {
      const entities = await loadSampleEntities(tag.id);
      for (const [entityType, entityId] of entities) {
        if (entityType === "task") {
          await invoke<void>("delete_task", { id: entityId }).catch(() => {});
        } else if (entityType === "chore") {
          await invoke<void>("delete_chore", { id: entityId }).catch(() => {});
        } else if (entityType === "event") {
          await invoke<void>("delete_event", { id: entityId }).catch(() => {});
        }
      }
      // Delete the tag itself — cascades tag_link rows.
      await invoke<void>("tag_delete", { id: tag.id }).catch(() => {});
      await refresh();
    } finally {
      setDeleting(false);
    }
  };

  if (!tag || count === 0) return null;

  return (
    <div style={{
      padding: 10, borderRadius: "var(--radius-lg)", background: "var(--surface)",
      border: "1px solid var(--hairline-strong)", display: "flex", justifyContent: "space-between",
      alignItems: "center", fontSize: 12,
    }}>
      <span style={{ color: "var(--ink)" }}>
        {count} sample item{count === 1 ? "" : "s"} still here from setup.
      </span>
      <button onClick={deleteAll} disabled={deleting} style={{ fontSize: 12 }}>
        {deleting ? "Removing…" : "Delete sample data"}
      </button>
    </div>
  );
}
