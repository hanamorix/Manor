import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { useSafetyStore } from "../../lib/safety/state";
import {
  trashList,
  trashRestore,
  trashPermanentDelete,
  trashEmptyAll,
} from "../../lib/safety/ipc";
import {
  COLOR_DANGER,
  TEXT_MUTED,
  dangerButton,
  settingsListRow,
} from "../Settings/styles";
import { SectionLabel } from "../../lib/ui";

function daysAgo(unix: number): string {
  const diff = Date.now() / 1000 - unix;
  const days = Math.floor(diff / 86400);
  if (days === 0) return "today";
  if (days === 1) return "yesterday";
  return `${days}d ago`;
}

export default function TrashView() {
  const { trashEntries, setTrashEntries } = useSafetyStore();
  const [loading, setLoading] = useState(true);
  const [emptying, setEmptying] = useState(false);
  const [confirmEmpty, setConfirmEmpty] = useState(false);

  const refresh = async () => {
    setLoading(true);
    try {
      setTrashEntries(await trashList());
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const grouped = trashEntries.reduce<Record<string, typeof trashEntries>>((acc, e) => {
    (acc[e.entity_type] ??= []).push(e);
    return acc;
  }, {});

  return (
    <section style={{ padding: 16 }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 12,
        }}
      >
        <h2 style={{ margin: 0, fontSize: 15, color: "var(--ink)" }}>
          Trash ({trashEntries.length})
        </h2>
        {trashEntries.length > 0 &&
          (confirmEmpty ? (
            <div style={{ display: "flex", gap: 8 }}>
              <button onClick={() => setConfirmEmpty(false)}>Cancel</button>
              <button
                onClick={async () => {
                  setEmptying(true);
                  try {
                    await trashEmptyAll();
                    await refresh();
                    setConfirmEmpty(false);
                  } finally {
                    setEmptying(false);
                  }
                }}
                disabled={emptying}
                style={dangerButton}
              >
                {emptying ? "Erasing…" : "Confirm empty"}
              </button>
            </div>
          ) : (
            <button onClick={() => setConfirmEmpty(true)}>Empty Trash</button>
          ))}
      </div>

      {loading && <div style={{ color: TEXT_MUTED }}>Loading…</div>}
      {!loading && trashEntries.length === 0 && (
        <div style={{ color: TEXT_MUTED }}>Trash is empty.</div>
      )}

      {Object.entries(grouped).map(([type, entries]) => (
        <div key={type} style={{ marginBottom: 16 }}>
          <SectionLabel icon={Trash2}>
            {type.replace("_", " ")} ({entries.length})
          </SectionLabel>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {entries.map((e) => (
              <div
                key={`${e.entity_type}-${e.entity_id}`}
                style={{
                  ...settingsListRow,
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                }}
              >
                <div>
                  <div style={{ color: "var(--ink)", fontSize: 13 }}>
                    {e.title || "(untitled)"}
                  </div>
                  <div style={{ fontSize: 11, color: TEXT_MUTED }}>
                    deleted {daysAgo(e.deleted_at)}
                  </div>
                </div>
                <div style={{ display: "flex", gap: 6 }}>
                  <button
                    onClick={async () => {
                      await trashRestore(e.entity_type, e.entity_id);
                      await refresh();
                    }}
                  >
                    Restore
                  </button>
                  <button
                    onClick={async () => {
                      await trashPermanentDelete(e.entity_type, e.entity_id);
                      await refresh();
                    }}
                    style={{ color: COLOR_DANGER }}
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      ))}
    </section>
  );
}
