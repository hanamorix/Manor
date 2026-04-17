import { useAssistantStore } from "../../lib/assistant/state";
import { expressionFor } from "../../lib/assistant/expressions";

const NATURAL_RATIO = 274 / 400; // intrinsic w/h of the avatar PNGs in material/

interface AvatarProps {
  /** Rendered height in px. Width is computed from the avatar's natural aspect ratio. */
  height?: number;
  onClick?: () => void;
}

export default function Avatar({ height = 96, onClick }: AvatarProps) {
  const state = useAssistantStore((s) => s.avatarState);
  const src = expressionFor(state);
  const width = Math.round(height * NATURAL_RATIO);

  const img = (
    <img
      src={src}
      alt="Manor"
      width={width}
      height={height}
      style={{
        width,
        height,
        transform: "scaleX(-1)",
        transition: "opacity 150ms ease-in-out",
        userSelect: "none",
        pointerEvents: "none",
      }}
      draggable={false}
    />
  );

  if (!onClick) return img;

  return (
    <button
      onClick={onClick}
      aria-label="Open conversation with Manor"
      style={{
        border: "none",
        background: "transparent",
        padding: 0,
        cursor: "pointer",
        display: "inline-block",
        lineHeight: 0,
      }}
    >
      {img}
    </button>
  );
}
