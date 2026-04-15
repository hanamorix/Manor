import Assistant from "./components/Assistant/Assistant";

export default function App() {
  return (
    <main
      style={{
        minHeight: "100vh",
        padding: "2rem",
        position: "relative",
      }}
    >
      <h1 style={{ fontWeight: 700, fontSize: 28 }}>Manor</h1>
      <p style={{ color: "rgba(0,0,0,0.6)" }}>
        Phase 2: she's here. Future views (Today, Chores, Meals) land in later phases.
      </p>

      <Assistant />
    </main>
  );
}
