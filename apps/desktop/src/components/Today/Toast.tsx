import { useEffect } from "react";
import { useTodayStore } from "../../lib/today/state";

export default function Toast() {
  const toast = useTodayStore((s) => s.toast);
  const clearToast = useTodayStore((s) => s.clearToast);

  useEffect(() => {
    if (!toast) return;
    const ms = Math.max(0, toast.expiresAt - Date.now());
    const id = setTimeout(() => clearToast(), ms);
    return () => clearTimeout(id);
  }, [toast, clearToast]);

  if (!toast) return null;

  return (
    <div
      role="status"
      style={{
        position: "fixed",
        bottom: 24,
        left: "50%",
        transform: "translateX(-50%)",
        background: "var(--scrim)",
        color: "var(--action-fg)",
        padding: "8px 16px",
        borderRadius: "var(--radius-pill)",
        fontSize: "var(--text-sm)",
        fontWeight: 600,
        boxShadow: "var(--shadow-md)",
        zIndex: 1100,
        animation: "bannerIn 200ms ease-out",
      }}
    >
      {toast.message}
    </div>
  );
}
