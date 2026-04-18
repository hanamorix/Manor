import { ImageOff } from "lucide-react";
import type { Recipe } from "../../lib/recipe/recipe-ipc";

interface Props {
  recipe: Recipe;
  heroSrc?: string;
  onClick: () => void;
}

export function RecipeCard({ recipe, heroSrc, onClick }: Props) {
  const meta = [
    (recipe.prep_time_mins != null || recipe.cook_time_mins != null)
      ? `${(recipe.prep_time_mins ?? 0) + (recipe.cook_time_mins ?? 0)}m`
      : null,
    recipe.servings != null ? `${recipe.servings}p` : null,
  ].filter(Boolean).join(" · ");

  return (
    <button
      onClick={onClick}
      style={{
        textAlign: "left",
        background: "var(--paper, #fcfcfc)",
        border: "1px solid var(--hairline, #efefef)",
        borderRadius: "var(--radius-lg, 6px)",
        padding: 0,
        cursor: "pointer",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        width: "100%",
      }}
    >
      <div style={{
        aspectRatio: "4 / 3",
        background: "var(--paper-muted, #f4f4f4)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}>
        {heroSrc ? (
          <img
            src={heroSrc}
            alt={recipe.title}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <ImageOff size={32} strokeWidth={1.4} color="var(--ink-soft, #6b6b6b)" />
        )}
      </div>
      <div style={{ padding: 12 }}>
        <div style={{
          fontSize: "var(--text-lg, 1rem)",
          fontWeight: 600,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
          color: "var(--ink, #1f1f1f)",
        }}>
          {recipe.title}
        </div>
        {meta && (
          <div style={{
            fontSize: "var(--text-xs, 0.75rem)",
            color: "var(--ink-soft, #6b6b6b)",
            marginTop: 4,
          }}>
            {meta}
          </div>
        )}
      </div>
    </button>
  );
}
