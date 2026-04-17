import { useWizardStore } from "../../lib/wizard/state";
import StepDataDir from "./StepDataDir";
import StepOllama from "./StepOllama";
import StepCalendar from "./StepCalendar";
import StepSample from "./StepSample";

const TITLES = ["Data directory", "Local brain", "Your calendar", "Sample data"];

export default function Wizard() {
  const step = useWizardStore((s) => s.step);

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--paper)",
        zIndex: 2000,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        padding: 48,
        overflowY: "auto",
      }}
    >
      <div
        style={{
          width: "100%",
          maxWidth: 520,
          display: "flex",
          flexDirection: "column",
          gap: 16,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div style={{ fontSize: 13, color: "rgba(0,0,0,0.55)", fontWeight: 500 }}>
            Setting up Manor
          </div>
          <div
            style={{
              flex: 1,
              height: 4,
              background: "var(--hairline)",
              borderRadius: 2,
              overflow: "hidden",
            }}
          >
            <div
              style={{
                width: `${(step / 4) * 100}%`,
                height: "100%",
                background: "var(--imessage-blue)",
                borderRadius: 2,
                transition: "width 200ms",
              }}
            />
          </div>
          <div
            style={{
              fontSize: 13,
              color: "rgba(0,0,0,0.55)",
              minWidth: 30,
              textAlign: "right",
            }}
          >
            {step}/4
          </div>
        </div>

        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 700, color: "var(--ink)" }}>
          {TITLES[step - 1]}
        </h1>

        <div
          style={{
            background: "#fff",
            border: "1px solid var(--hairline)",
            borderRadius: "var(--radius-lg)",
            boxShadow: "var(--shadow-sm)",
            padding: 24,
          }}
        >
          {step === 1 && <StepDataDir />}
          {step === 2 && <StepOllama />}
          {step === 3 && <StepCalendar />}
          {step === 4 && <StepSample />}
        </div>
      </div>
    </div>
  );
}
