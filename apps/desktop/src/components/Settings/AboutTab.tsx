import { useEffect, useState } from "react";
import { appVersion } from "../../lib/foundation/ipc";
import { TEXT_MUTED, TEXT_SECONDARY } from "./styles";

export default function AboutTab() {
  const [version, setVersion] = useState<string>("…");

  useEffect(() => {
    void appVersion().then(setVersion).catch(() => setVersion("(unknown)"));
  }, []);

  return (
    <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 16 }}>
      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>Manor</h2>
        <div style={{ fontSize: 13, color: "var(--ink)" }}>Version {version}</div>
        <div style={{ fontSize: 12, color: TEXT_MUTED, marginTop: 4 }}>
          A calm, local-first household assistant. Licensed AGPL-3.0.
        </div>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>Links</h2>
        <div style={{ fontSize: 13, display: "flex", flexDirection: "column", gap: 4 }}>
          <a
            href="https://github.com/hanamorix/Manor"
            target="_blank"
            rel="noreferrer"
            style={{ color: "var(--ink)" }}
          >
            GitHub repository
          </a>
          <a
            href="https://github.com/sponsors/hanamorix"
            target="_blank"
            rel="noreferrer"
            style={{ color: "var(--ink)" }}
          >
            Support development (GitHub Sponsors)
          </a>
        </div>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15, color: "var(--ink)" }}>
          How this app is sustained
        </h2>
        <div style={{ fontSize: 12, color: TEXT_SECONDARY, lineHeight: 1.5 }}>
          Manor is free, open-source, and local-first. It runs entirely on your Mac —
          no accounts, no telemetry, no data leaving your machine unless you configure
          a remote AI provider and explicitly opt in per call. Development is funded
          through GitHub Sponsors and optional cloud-backup subscriptions (v1.0+). If
          the project helps you, sponsoring keeps it alive.
        </div>
      </section>
    </div>
  );
}
