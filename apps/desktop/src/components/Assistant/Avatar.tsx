import { useAssistantStore } from "../../lib/assistant/state";
import { expressionFor } from "../../lib/assistant/expressions";

interface AvatarProps {
  size?: number;
  onClick?: () => void;
}

export default function Avatar({ size = 96, onClick }: AvatarProps) {
  const state = useAssistantStore((s) => s.avatarState);
  const src = expressionFor(state);

  return (
    <img
      src={src}
      alt="Manor"
      width={size}
      height={size}
      onClick={onClick}
      style={{
        width: size,
        height: size,
        cursor: onClick ? "pointer" : "default",
        transform: "scaleX(-1)",
        transition: "opacity 150ms ease-in-out",
        userSelect: "none",
      }}
      draggable={false}
    />
  );
}
