import { useEffect, useState } from "react";
import { Utensils, Ban } from "lucide-react";
import { useMealPlanStore } from "../../lib/meal_plan/meal-plan-state";
import { useHearthViewStore } from "../../lib/hearth/view-state";
import { settingGet } from "../../lib/foundation/ipc";
import * as recipeIpc from "../../lib/recipe/recipe-ipc";

export function TonightBand() {
  const { tonight, loadTonight } = useMealPlanStore();
  const { openRecipeDetail, setSubview } = useHearthViewStore();
  const [visible, setVisible] = useState<boolean>(true);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  useEffect(() => { void loadTonight(); }, [loadTonight]);
  useEffect(() => {
    void settingGet("hearth.show_tonight_band").then((v) => setVisible(v !== "false")).catch(() => {});
  }, []);
  useEffect(() => {
    const uuid = tonight?.recipe?.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void recipeIpc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [tonight?.recipe?.hero_attachment_uuid]);

  if (!visible) return null;

  const recipe = tonight?.recipe;
  const isGhost = recipe?.deleted_at != null;

  const planHearthAndWeek = () => { setSubview("this_week"); };

  const bandStyle: React.CSSProperties = {
    display: "flex", alignItems: "center", gap: 12,
    height: 56, padding: "0 16px",
    background: "var(--paper, #fff)",
    border: "1px solid var(--hairline, #e5e5e5)",
    borderRadius: 6,
  };

  if (!recipe) {
    return (
      <div style={bandStyle}>
        <Utensils size={18} strokeWidth={1.6} color="var(--ink-soft, #999)" />
        <span style={{ flex: 1, color: "var(--ink-soft, #999)" }}>No dinner planned</span>
        <button type="button" onClick={planHearthAndWeek}>Plan one →</button>
      </div>
    );
  }

  if (isGhost) {
    return (
      <div style={bandStyle}>
        <Ban size={18} strokeWidth={1.6} color="var(--ink-soft, #999)" />
        <span style={{ flex: 1, color: "var(--ink-soft, #999)" }}>Recipe deleted — restore or replace?</span>
        <button type="button" onClick={async () => {
          if (recipe) { await recipeIpc.restore(recipe.id); await loadTonight(); }
        }}>Restore</button>
        <button type="button" onClick={planHearthAndWeek}>Replace →</button>
      </div>
    );
  }

  const meta = [
    recipe.cook_time_mins != null || recipe.prep_time_mins != null
      ? `${(recipe.prep_time_mins ?? 0) + (recipe.cook_time_mins ?? 0)}m`
      : null,
    recipe.servings != null ? `serves ${recipe.servings}` : null,
  ].filter(Boolean).join(" · ");

  return (
    <div
      onClick={() => openRecipeDetail(recipe.id)}
      style={{
        ...bandStyle,
        padding: "0 12px",
        cursor: "pointer",
      }}
    >
      <div style={{ width: 40, height: 40, borderRadius: 4, overflow: "hidden",
                    background: "var(--paper-muted, #f5f5f5)",
                    display: "flex", alignItems: "center", justifyContent: "center",
                    flexShrink: 0 }}>
        {heroSrc ? <img src={heroSrc} alt="" style={{ width: "100%", height: "100%", objectFit: "cover" }} />
                 : <Utensils size={16} strokeWidth={1.6} color="var(--ink-soft, #999)" />}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 600,
                      whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          Tonight — {recipe.title}
        </div>
        {meta && <div style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>{meta}</div>}
      </div>
      <span style={{ fontSize: 12, color: "var(--ink-soft, #999)" }}>View recipe →</span>
    </div>
  );
}
