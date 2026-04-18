import { useEffect, useState } from "react";
import { Plus, Ban, MoreHorizontal, X, ImageOff } from "lucide-react";
import type { MealPlanEntryWithRecipe } from "../../../lib/meal_plan/meal-plan-ipc";
import * as recipeIpc from "../../../lib/recipe/recipe-ipc";

interface Props {
  entry: MealPlanEntryWithRecipe;
  isToday: boolean;
  onEmptyClick: () => void;
  onFilledClick: (recipeId: string) => void;
  onGhostClick: (entry: MealPlanEntryWithRecipe) => void;
  onSwap: () => void;
  onRemove: () => void;
}

export function DaySlotCard(props: Props) {
  const { entry, isToday, onEmptyClick, onFilledClick, onGhostClick, onSwap, onRemove } = props;
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  useEffect(() => {
    const uuid = entry.recipe?.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void recipeIpc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [entry.recipe?.hero_attachment_uuid]);

  const columnBg = isToday ? "var(--paper-muted, #f5f5f5)" : "transparent";

  if (!entry.recipe) {
    return (
      <button
        type="button"
        onClick={onEmptyClick}
        style={{
          width: "100%",
          aspectRatio: "4/5",
          background: columnBg,
          border: "1px dashed var(--hairline, #e5e5e5)",
          borderRadius: 6,
          cursor: "pointer",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--ink-soft, #999)",
        }}
      >
        <Plus size={18} strokeWidth={1.6} />
        <span style={{ fontSize: 12, marginTop: 4 }}>Plan a meal</span>
      </button>
    );
  }

  const recipe = entry.recipe;
  const isGhost = recipe.deleted_at != null;

  if (isGhost) {
    return (
      <button
        type="button"
        onClick={() => onGhostClick(entry)}
        style={{
          width: "100%",
          aspectRatio: "4/5",
          background: columnBg,
          border: "1px solid var(--hairline, #e5e5e5)",
          borderRadius: 6,
          cursor: "pointer",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--ink-soft, #999)",
          padding: 8,
          textAlign: "center",
        }}
      >
        <Ban size={22} strokeWidth={1.4} />
        <div style={{ fontSize: 12, marginTop: 6 }}>Recipe deleted</div>
        <div style={{ fontSize: 11, marginTop: 2 }}>Tap to restore or unplan</div>
      </button>
    );
  }

  const pillStyle: React.CSSProperties = {
    background: "rgba(255,255,255,0.9)",
    border: "1px solid var(--hairline, #e5e5e5)",
    borderRadius: 4,
    padding: "2px 4px",
    cursor: "pointer",
  };

  return (
    <div
      onClick={() => onFilledClick(recipe.id)}
      style={{
        width: "100%",
        aspectRatio: "4/5",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        overflow: "hidden",
        position: "relative",
        display: "flex",
        flexDirection: "column",
        cursor: "pointer",
      }}
    >
      <div style={{
        aspectRatio: "4/3",
        background: "var(--paper-muted, #f5f5f5)",
        display: "flex", alignItems: "center", justifyContent: "center",
      }}>
        {heroSrc ? (
          <img src={heroSrc} alt={recipe.title}
            style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <ImageOff size={20} strokeWidth={1.4} color="var(--ink-soft, #999)" />
        )}
      </div>
      <div style={{ padding: 8, fontSize: 12, fontWeight: 600,
                    whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
        {recipe.title}
      </div>
      <div style={{ position: "absolute", top: 4, right: 4, display: "flex", gap: 4 }}>
        <button type="button" aria-label="Swap recipe"
          onClick={(e) => { e.stopPropagation(); onSwap(); }} style={pillStyle}>
          <MoreHorizontal size={12} strokeWidth={1.8} />
        </button>
        <button type="button" aria-label="Remove"
          onClick={(e) => { e.stopPropagation(); onRemove(); }} style={pillStyle}>
          <X size={12} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
