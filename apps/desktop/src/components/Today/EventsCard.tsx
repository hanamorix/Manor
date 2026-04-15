const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
};

const sectionHeader: React.CSSProperties = {
  fontSize: 11,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: "rgba(0,0,0,0.55)",
  fontWeight: 700,
  margin: 0,
  marginBottom: 8,
};

export default function EventsCard() {
  return (
    <div style={cardStyle}>
      <p style={sectionHeader}>Events</p>
      <p style={{ fontStyle: "italic", color: "rgba(0,0,0,0.5)", margin: 0, fontSize: 13 }}>
        No calendar connected. Coming next phase.
      </p>
    </div>
  );
}
