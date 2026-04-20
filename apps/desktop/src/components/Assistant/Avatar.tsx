import manorFace from "../../assets/avatars/manor_face.png";

interface AvatarProps {
  /** Rendered height in px. Width scales from the image's intrinsic aspect ratio. */
  height?: number;
  onClick?: () => void;
}

export default function Avatar({ height = 72, onClick }: AvatarProps) {
  const img = (
    <img
      src={manorFace}
      alt="Manor"
      style={{
        height,
        width: "auto",
        transform: "scaleX(-1)",
        userSelect: "none",
        pointerEvents: "none",
        display: "block",
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
