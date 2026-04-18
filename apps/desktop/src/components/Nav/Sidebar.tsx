import {
  Home, LayoutDashboard, Sparkles, LayoutGrid, Wallet, Wrench, UtensilsCrossed,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useNavStore, type View } from "../../lib/nav";
import { useSettingsStore } from "../../lib/settings/state";

const SIDEBAR_WIDTH = 58;

const railStyle: React.CSSProperties = {
  width: SIDEBAR_WIDTH,
  background: "var(--paper-muted)",
  borderRight: "1px solid var(--hairline)",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  padding: "14px 0 12px",
  gap: 6,
  flexShrink: 0,
  height: "100vh",
};

const manorButtonStyle: React.CSSProperties = {
  width: 32,
  height: 32,
  borderRadius: "var(--radius-sm, 6px)",
  background: "var(--ink)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  boxShadow: "none",
  marginBottom: 10,
  border: "none",
  cursor: "pointer",
  padding: 0,
  fontFamily: "inherit",
};

const iconWrapStyle: React.CSSProperties = {
  position: "relative",
  width: 38,
  height: 38,
  borderRadius: "var(--radius-lg)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "pointer",
  background: "transparent",
};

interface NavIconProps {
  view: View;
  icon: LucideIcon;
  title: string;
}

function NavIcon({ view, icon: Icon, title }: NavIconProps) {
  const current = useNavStore((s) => s.view);
  const setView = useNavStore((s) => s.setView);
  const active = current === view;
  return (
    <div
      role="button"
      tabIndex={0}
      aria-label={title}
      aria-current={active ? "page" : undefined}
      title={title}
      onClick={() => setView(view)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          setView(view);
        }
      }}
      style={iconWrapStyle}
    >
      {active && (
        <span
          style={{
            position: "absolute",
            left: 0,
            top: "50%",
            transform: "translateY(-50%)",
            width: 2,
            height: 16,
            background: "var(--ink)",
            borderRadius: 1,
          }}
        />
      )}
      <Icon
        size={20}
        strokeWidth={1.8}
        color={active ? "var(--ink)" : "var(--ink-faint)"}
      />
    </div>
  );
}

export default function Sidebar() {
  const setModalOpen = useSettingsStore((s) => s.setModalOpen);
  return (
    <nav style={railStyle} aria-label="Primary navigation">
      <button
        type="button"
        onClick={() => setModalOpen(true)}
        title="Settings"
        aria-label="Open settings"
        style={manorButtonStyle}
      >
        <Home size={18} strokeWidth={1.8} color="var(--paper)" />
      </button>
      <NavIcon view="today" icon={LayoutDashboard} title="Today" />
      <NavIcon view="chores" icon={Sparkles} title="Chores" />
      <NavIcon view="timeblocks" icon={LayoutGrid} title="Time Blocks" />
      <NavIcon view="ledger" icon={Wallet} title="Ledger" />
      <NavIcon view="bones" icon={Wrench} title="Bones" />
      <NavIcon view="hearth" icon={UtensilsCrossed} title="Hearth" />
      <div style={{ flex: 1 }} />
    </nav>
  );
}
