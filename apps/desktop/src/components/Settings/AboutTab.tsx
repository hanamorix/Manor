import { useEffect, useState } from "react";
import { appVersion } from "../../lib/foundation/ipc";

export default function AboutTab() {
  const [version, setVersion] = useState<string>("…");

  useEffect(() => {
    void appVersion().then(setVersion).catch(() => setVersion("(unknown)"));
  }, []);

  return (
    <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 16 }}>
      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Manor</h2>
        <div style={{ fontSize: 13 }}>Version {version}</div>
        <div style={{ fontSize: 12, color: "#888", marginTop: 4 }}>
          A calm, local-first household assistant. Licensed AGPL-3.0.
        </div>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Links</h2>
        <div style={{ fontSize: 13, display: "flex", flexDirection: "column", gap: 4 }}>
          <a href="https://github.com/hanamorix/Manor" target="_blank" rel="noreferrer">
            GitHub repository
          </a>
          <a href="https://github.com/sponsors/hanamorix" target="_blank" rel="noreferrer">
            Support development (GitHub Sponsors)
          </a>
        </div>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>How this app is sustained</h2>
        <div style={{ fontSize: 12, color: "#aaa", lineHeight: 1.5 }}>
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
