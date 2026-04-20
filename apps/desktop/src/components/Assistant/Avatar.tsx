import manorFace from "../../assets/avatars/manor_face.png";

const NATURAL_RATIO = 274 / 400; // intrinsic w/h ratio preserved from old avatars

interface AvatarProps {
  /** Rendered height in px. Width is computed from the avatar's natural aspect ratio. */
  height?: number;
  onClick?: () => void;
}

export default function Avatar({ height = 72, onClick }: AvatarProps) {
  const width = Math.round(height * NATURAL_RATIO);

  const img = (
    <img
      src={manorFace}
      alt="Manor"
      width={width}
      height={height}
      style={{
        width,
        height,
        transform: "scaleX(-1)",
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
