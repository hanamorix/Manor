import { useEffect, useState } from "react";
import {
  householdGet, householdSetOwner, householdSetWorkingHours,
  personList, personAdd, personDelete,
  settingGet, settingSet,
  type Household, type Person, type WorkingHours,
} from "../../lib/foundation/ipc";

const DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

function WeatherLocationInput() {
  const [value, setValue] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    void settingGet("today.weather_location").then((v) => {
      setValue(v ?? "");
      setLoaded(true);
    });
  }, []);

  const save = async () => {
    setSaving(true);
    setMessage(null);
    try {
      await settingSet("today.weather_location", value.trim());
      setMessage("Saved. Weather refreshes on the next Today load.");
    } catch (e) {
      setMessage(`Failed: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  if (!loaded) return <div style={{ fontSize: 12, color: "#666" }}>…</div>;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <input
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder="e.g. London, NW1 (leave blank for auto-detect)"
        style={{ width: "100%" }}
      />
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <button onClick={save} disabled={saving}>
          {saving ? "Saving…" : "Save location"}
        </button>
        {message && <span style={{ fontSize: 11, color: message.includes("Failed") ? "#f66" : "#6f6" }}>{message}</span>}
      </div>
      <div style={{ fontSize: 11, color: "#666" }}>
        Uses wttr.in — if blank, location is inferred from your IP.
      </div>
    </div>
  );
}

export default function HouseholdTab() {
  const [household, setHousehold] = useState<Household | null>(null);
  const [people, setPeople] = useState<Person[]>([]);
  const [newName, setNewName] = useState("");

  const refresh = async () => {
    setHousehold(await householdGet());
    setPeople(await personList());
  };

  useEffect(() => { void refresh(); }, []);

  const onOwnerChange = async (id: string) => {
    const v = id === "" ? null : parseInt(id, 10);
    const h = await householdSetOwner(v);
    setHousehold(h);
  };

  const onHoursChange = async (day: string, start: string, end: string) => {
    if (!household) return;
    const next: WorkingHours = { ...household.working_hours };
    const startN = parseInt(start, 10);
    const endN = parseInt(end, 10);
    if (isNaN(startN) || isNaN(endN) || startN >= endN) {
      next[day] = []; // invalid → rest day
    } else {
      next[day] = [startN, endN];
    }
    const h = await householdSetWorkingHours(next);
    setHousehold(h);
  };

  const onAddPerson = async () => {
    if (!newName.trim()) return;
    await personAdd({ name: newName.trim(), kind: "member" });
    setNewName("");
    await refresh();
  };

  if (!household) return <div style={{ padding: 16 }}>Loading…</div>;

  return (
    <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 16 }}>
      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Household owner</h2>
        <select
          value={household.owner_person_id?.toString() ?? ""}
          onChange={(e) => void onOwnerChange(e.target.value)}
        >
          <option value="">(unset)</option>
          {people.map((p) => (
            <option key={p.id} value={p.id}>{p.name} ({p.kind})</option>
          ))}
        </select>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Working hours</h2>
        <table style={{ fontSize: 13, borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", padding: "4px 8px" }}>Day</th>
              <th style={{ textAlign: "left", padding: "4px 8px" }}>Start</th>
              <th style={{ textAlign: "left", padding: "4px 8px" }}>End</th>
            </tr>
          </thead>
          <tbody>
            {DAYS.map((d) => {
              const pair = household.working_hours[d] ?? [];
              const [startV, endV] = pair.length === 2 ? pair : [null, null];
              return (
                <tr key={d}>
                  <td style={{ padding: "4px 8px", textTransform: "capitalize" }}>{d}</td>
                  <td>
                    <input type="number" min={0} max={23}
                           defaultValue={startV ?? ""}
                           placeholder="off"
                           style={{ width: 60 }}
                           onBlur={(e) => {
                             const end = pair.length === 2 ? pair[1].toString() : "";
                             void onHoursChange(d, e.target.value, end);
                           }} />
                  </td>
                  <td>
                    <input type="number" min={1} max={24}
                           defaultValue={endV ?? ""}
                           placeholder="off"
                           style={{ width: 60 }}
                           onBlur={(e) => {
                             const start = pair.length === 2 ? pair[0].toString() : "";
                             void onHoursChange(d, start, e.target.value);
                           }} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        <div style={{ fontSize: 11, color: "#666", marginTop: 4 }}>
          Leave blank or set start ≥ end to mark a rest day.
        </div>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>People</h2>
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          {people.map((p) => (
            <div key={p.id}
                 style={{ display: "flex", justifyContent: "space-between",
                          padding: 6, borderRadius: 4, background: "#151515" }}>
              <div>
                <div style={{ fontSize: 13 }}>{p.name}</div>
                <div style={{ fontSize: 11, color: "#666" }}>{p.kind}</div>
              </div>
              <button onClick={async () => { await personDelete(p.id); await refresh(); }}
                      style={{ fontSize: 12 }}>
                Remove
              </button>
            </div>
          ))}
        </div>
        <div style={{ marginTop: 8, display: "flex", gap: 6 }}>
          <input value={newName} onChange={(e) => setNewName(e.target.value)}
                 placeholder="Add a member" />
          <button onClick={onAddPerson} disabled={!newName.trim()}>Add</button>
        </div>
      </section>

      <section>
        <h2 style={{ margin: "0 0 8px 0", fontSize: 15 }}>Weather location</h2>
        <WeatherLocationInput />
      </section>
    </div>
  );
}
