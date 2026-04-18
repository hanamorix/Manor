import { useEffect } from "react";
import { HearthSubNav } from "./HearthSubNav";
import { RecipesView } from "./RecipesView";
import { ThisWeekView } from "./ThisWeek/ThisWeekView";
import { StaplesView } from "./Staples/StaplesView";
import { useHearthViewStore } from "../../lib/hearth/view-state";

export function HearthTab() {
  const { subview, hydrate, hydrated } = useHearthViewStore();
  useEffect(() => { void hydrate(); }, [hydrate]);

  if (!hydrated) return null;

  return (
    <div style={{ padding: 32, maxWidth: 1200, margin: "0 auto" }}>
      <HearthSubNav />
      {subview === "recipes" && <RecipesView />}
      {subview === "this_week" && <ThisWeekView />}
      {subview === "staples" && <StaplesView />}
    </div>
  );
}
