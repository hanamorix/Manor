// Stub — T15 will implement the real connect/reconnect drawer.
export function ConnectBankDrawer(_: {
  mode: { kind: "connect" } | { kind: "reconnect"; account_id: number };
  onClose: () => void;
}) {
  return null;
}
