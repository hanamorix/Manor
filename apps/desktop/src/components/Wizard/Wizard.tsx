import { useWizardStore } from "../../lib/wizard/state";
import StepDataDir from "./StepDataDir";
import StepOllama from "./StepOllama";
import StepCalendar from "./StepCalendar";

const TITLES = ["Data directory", "Local brain", "Your calendar", "Sample data"];

export default function Wizard() {
  const step = useWizardStore((s) => s.step);

  return (
    <div style={{
      position: "fixed", inset: 0, background: "#0b0b0b", zIndex: 2000,
      display: "flex", flexDirection: "column", alignItems: "center", padding: 48,
    }}>
      <div style={{
        width: "100%", maxWidth: 520, display: "flex", flexDirection: "column", gap: 16,
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <div style={{ fontSize: 13, color: "#888" }}>Setting up Manor</div>
          <div style={{ flex: 1, height: 4, background: "#222", borderRadius: 2 }}>
            <div
              style={{
                width: `${(step / 4) * 100}%`,
                height: "100%", background: "var(--imessage-blue, #3a95f7)", borderRadius: 2,
                transition: "width 200ms",
              }}
            />
          </div>
          <div style={{ fontSize: 13, color: "#888", minWidth: 30, textAlign: "right" }}>
            {step}/4
          </div>
        </div>

        <h1 style={{ margin: 0, fontSize: 22 }}>{TITLES[step - 1]}</h1>

        <div style={{
          background: "#151515", border: "1px solid #222", borderRadius: 8, padding: 24,
        }}>
          {step === 1 && <StepDataDir />}
          {step === 2 && <StepOllama />}
          {step === 3 && <StepCalendar />}
          {step === 4 && <StepPlaceholder label="Step 4 — filling in Task 6" />}
        </div>
      </div>
    </div>
  );
}

function StepPlaceholder({ label }: { label: string }) {
  return <div style={{ padding: 16, color: "#888" }}>{label}</div>;
}
