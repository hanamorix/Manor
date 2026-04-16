import { useEffect, useState } from "react";
import SettingsCog from "../Settings/SettingsCog";
import { weatherCurrent, type Weather } from "../../lib/today/ipc";

const cardStyle: React.CSSProperties = {
  background: "var(--paper)",
  border: "1px solid var(--hairline)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-sm)",
  padding: "16px 18px",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
};

const FORMATTER = new Intl.DateTimeFormat(undefined, {
  weekday: "long",
  day: "numeric",
  month: "long",
});

function tzAbbrev(): string {
  const parts = new Intl.DateTimeFormat(undefined, { timeZoneName: "short" })
    .formatToParts(new Date());
  return parts.find((p) => p.type === "timeZoneName")?.value ?? "";
}

export default function HeaderCard() {
  const [now, setNow] = useState(new Date());
  const [weather, setWeather] = useState<Weather | null>(null);

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    void weatherCurrent().then(setWeather).catch(() => setWeather(null));
    const id = setInterval(() => {
      void weatherCurrent().then(setWeather).catch(() => {});
    }, 30 * 60_000);
    return () => clearInterval(id);
  }, []);

  const dateLabel = FORMATTER.format(now);
  const time = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
  const tz = tzAbbrev();

  return (
    <div style={cardStyle}>
      <div>
        <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>Today</h1>
        <div style={{ fontSize: 13, color: "rgba(0,0,0,0.55)" }}>{dateLabel}</div>
        {weather && (
          <div style={{ fontSize: 12, color: "rgba(0,0,0,0.55)", marginTop: 2 }}>
            {weather.emoji} {weather.temp_c}°C, {weather.condition}
            {weather.location && ` — ${weather.location}`}
          </div>
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <div
          style={{
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            fontSize: 12,
            color: "rgba(0,0,0,0.55)",
          }}
          aria-label="current local time"
        >
          {time} {tz}
        </div>
        <SettingsCog />
      </div>
    </div>
  );
}
