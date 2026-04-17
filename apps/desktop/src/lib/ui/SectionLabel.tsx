import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  icon: LucideIcon;
  children: ReactNode;
  action?: ReactNode;
}

export function SectionLabel({ icon: Icon, children, action }: Props) {
  return (
    <header
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 8,
        marginBottom: 8,
        color: "var(--ink-soft)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <Icon size={14} strokeWidth={1.8} />
        <span style={{ fontSize: "var(--text-xs)", fontWeight: 500 }}>
          {children}
        </span>
      </div>
      {action}
    </header>
  );
}
