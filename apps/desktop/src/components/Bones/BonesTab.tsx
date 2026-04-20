import { useEffect } from "react";
import { BonesSubNav } from "./BonesSubNav";
import { AssetsView } from "./AssetsView";
import { DueSoonView } from "./DueSoon/DueSoonView";
import { SpendView } from "./Spend/SpendView";
import { useBonesViewStore } from "../../lib/bones/view-state";

export function BonesTab() {
  const { subview, hydrate, hydrated } = useBonesViewStore();
  useEffect(() => { void hydrate(); }, [hydrate]);
  if (!hydrated) return null;
  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <BonesSubNav />
      {subview === "assets"   && <AssetsView />}
      {subview === "due_soon" && <DueSoonView />}
      {subview === "spend"    && <SpendView />}
    </div>
  );
}
