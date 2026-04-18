import { useEffect } from "react";
import { useAssetStore } from "../../lib/asset/state";

export function BonesTab() {
  const { assets, loadStatus, load } = useAssetStore();
  useEffect(() => { void load(); }, [load]);

  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <h1 style={{ fontSize: 24, fontWeight: 600, margin: 0 }}>Assets</h1>
      {loadStatus.kind === "loading" && <p style={{ color: "var(--ink-soft, #999)" }}>Loading…</p>}
      {loadStatus.kind === "error" && <p style={{ color: "var(--ink-danger, #b00020)" }}>{loadStatus.message}</p>}
      {loadStatus.kind === "idle" && assets.length === 0 && (
        <p style={{ color: "var(--ink-soft, #999)" }}>Your asset registry is empty.</p>
      )}
      {loadStatus.kind === "idle" && assets.length > 0 && (
        <ul>{assets.map(a => <li key={a.id}>{a.name}</li>)}</ul>
      )}
    </div>
  );
}
