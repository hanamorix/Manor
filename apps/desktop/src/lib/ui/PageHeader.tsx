import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  icon: LucideIcon;
  title: string;
  subtitle?: ReactNode;
  meta?: ReactNode;
}

export function PageHeader({ icon: Icon, title, subtitle, meta }: Props) {
  return (
    <header
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "baseline",
        marginBottom: 18,
        paddingBottom: 18,
      }}
    >
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <Icon size={22} strokeWidth={1.8} color="var(--ink)" />
          <h1
            style={{
              fontSize: "var(--text-xl)",
              fontWeight: 600,
              letterSpacing: "-0.015em",
              margin: 0,
              color: "var(--ink)",
            }}
          >
            {title}
          </h1>
        </div>
        {subtitle && (
          <div
            className="num"
            style={{
              fontSize: "var(--text-xs)",
              color: "var(--ink-soft)",
              marginTop: 2,
              marginLeft: 32,
            }}
          >
            {subtitle}
          </div>
        )}
      </div>
      {meta && (
        <div style={{ fontSize: "var(--text-xs)", color: "var(--ink-soft)" }}>
          {meta}
        </div>
      )}
    </header>
  );
}
