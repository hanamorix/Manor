import { useNavStore, type View } from "../../lib/nav";

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

const avatarStyle: React.CSSProperties = {
  width: 32,
  height: 32,
  borderRadius: "50%",
  background: "linear-gradient(135deg, #FFC15C 0%, #FF8800 100%)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: 16,
  boxShadow: "0 2px 6px rgba(255,136,0,0.3)",
  marginBottom: 10,
};

const iconStyle = (active: boolean): React.CSSProperties => ({
  width: 38,
  height: 38,
  borderRadius: 10,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: 17,
  cursor: "pointer",
  background: active ? "var(--paper)" : "transparent",
  boxShadow: active ? "0 1px 4px rgba(20,20,30,0.1)" : "none",
  color: active ? "var(--imessage-blue)" : "rgba(20,20,30,0.35)",
  transition: "background 0.15s, color 0.15s",
});

interface NavIconProps {
  view: View;
  icon: string;
  title: string;
}

function NavIcon({ view, icon, title }: NavIconProps) {
  const current = useNavStore((s) => s.view);
  const setView = useNavStore((s) => s.setView);
  const active = current === view;
  return (
    <div
      role="button"
      aria-label={title}
      aria-current={active ? "page" : undefined}
      title={title}
      onClick={() => setView(view)}
      style={iconStyle(active)}
    >
      {icon}
    </div>
  );
}

export default function Sidebar() {
  return (
    <nav style={railStyle} aria-label="Primary navigation">
      <div style={avatarStyle} aria-hidden="true">🌸</div>
      <NavIcon view="today" icon="🏠" title="Today" />
      <NavIcon view="chores" icon="🧹" title="Chores" />
      <NavIcon view="timeblocks" icon="⏱" title="Time Blocks" />
      <div style={{ flex: 1 }} />
    </nav>
  );
}
