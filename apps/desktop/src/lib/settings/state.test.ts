import { describe, it, expect, beforeEach } from "vitest";
import { useSettingsStore } from "./state";
import type { CalendarAccount } from "./ipc";

const sampleAccount = (overrides: Partial<CalendarAccount> = {}): CalendarAccount => ({
  id: 1,
  display_name: "iCloud",
  server_url: "https://caldav.icloud.com",
  username: "a@b.c",
  last_synced_at: null,
  last_error: null,
  created_at: Date.now(),
  default_calendar_url: null,
  ...overrides,
});

describe("useSettingsStore", () => {
  beforeEach(() => useSettingsStore.setState(useSettingsStore.getInitialState(), true));

  it("modal is closed by default", () => {
    expect(useSettingsStore.getState().modalOpen).toBe(false);
  });

  it("setModalOpen toggles", () => {
    useSettingsStore.getState().setModalOpen(true);
    expect(useSettingsStore.getState().modalOpen).toBe(true);
    useSettingsStore.getState().setModalOpen(false);
    expect(useSettingsStore.getState().modalOpen).toBe(false);
  });

  it("setAccounts replaces", () => {
    const a = sampleAccount({ id: 1 });
    const b = sampleAccount({ id: 2 });
    useSettingsStore.getState().setAccounts([a, b]);
    expect(useSettingsStore.getState().accounts).toEqual([a, b]);
  });

  it("upsertAccount replaces existing or appends new", () => {
    const a = sampleAccount({ id: 1, display_name: "old" });
    const aPrime = sampleAccount({ id: 1, display_name: "new" });
    const b = sampleAccount({ id: 2 });
    useSettingsStore.getState().setAccounts([a]);
    useSettingsStore.getState().upsertAccount(aPrime);
    expect(useSettingsStore.getState().accounts[0].display_name).toBe("new");
    useSettingsStore.getState().upsertAccount(b);
    expect(useSettingsStore.getState().accounts).toHaveLength(2);
  });

  it("removeAccount drops by id", () => {
    const a = sampleAccount({ id: 1 });
    const b = sampleAccount({ id: 2 });
    useSettingsStore.getState().setAccounts([a, b]);
    useSettingsStore.getState().removeAccount(1);
    expect(useSettingsStore.getState().accounts).toEqual([b]);
  });

  it("markSyncing and markSynced update the set", () => {
    useSettingsStore.getState().markSyncing(42);
    expect(useSettingsStore.getState().syncingAccountIds.has(42)).toBe(true);
    useSettingsStore.getState().markSynced(42);
    expect(useSettingsStore.getState().syncingAccountIds.has(42)).toBe(false);
  });
});
