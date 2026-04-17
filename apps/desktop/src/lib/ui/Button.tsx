import type { LucideIcon } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  icon?: LucideIcon;
  children: ReactNode;
}

export function Button({
  variant = "primary",
  icon: Icon,
  children,
  style,
  ...rest
}: Props) {
  const base = {
    fontFamily: "inherit",
    fontSize: "var(--text-xs)",
    padding: "6px 11px",
    borderRadius: "var(--radius-md)",
    cursor: "pointer",
    fontWeight: 500,
    display: "inline-flex",
    alignItems: "center",
    gap: 5,
  } as const;

  const variantStyle =
    variant === "primary"
      ? {
          border: "none",
          background: "var(--action-bg)",
          color: "var(--action-fg)",
        }
      : {
          border: "1px solid var(--action-secondary-border)",
          background: "transparent",
          color: "var(--ink)",
        };

  return (
    <button {...rest} style={{ ...base, ...variantStyle, ...style }}>
      {Icon && <Icon size={12} strokeWidth={2.2} />}
      {children}
    </button>
  );
}
