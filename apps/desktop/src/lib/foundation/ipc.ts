import { invoke } from "@tauri-apps/api/core";

export async function settingGet(key: string): Promise<string | null> {
  return invoke<string | null>("setting_get", { key });
}

export async function settingSet(key: string, value: string): Promise<void> {
  return invoke<void>("setting_set", { key, value });
}

export async function settingListPrefixed(prefix: string): Promise<Array<[string, string]>> {
  return invoke<Array<[string, string]>>("setting_list_prefixed", { prefix });
}

export interface Person {
  id: number;
  name: string;
  kind: string;
  email: string | null;
  phone: string | null;
  note: string | null;
  created_at: number;
  updated_at: number;
}

export interface PersonArgs {
  name: string;
  kind: string;
  email?: string;
  phone?: string;
  note?: string;
}

export async function personList(): Promise<Person[]> {
  return invoke<Person[]>("person_list");
}
export async function personAdd(args: PersonArgs): Promise<Person> {
  return invoke<Person>("person_add", { args });
}
export async function personDelete(id: number): Promise<void> {
  return invoke<void>("person_delete", { id });
}

export type WorkingHours = Record<string, number[]>; // day -> [start, end] (empty = rest)

export interface DndWindow {
  day: string;
  start_hour: number;
  end_hour: number;
}

export interface Household {
  owner_person_id: number | null;
  working_hours: WorkingHours;
  dnd_windows: DndWindow[];
  created_at: number;
  updated_at: number;
}

export async function householdGet(): Promise<Household> {
  return invoke<Household>("household_get");
}
export async function householdSetOwner(ownerPersonId: number | null): Promise<Household> {
  return invoke<Household>("household_set_owner", { ownerPersonId });
}
export async function householdSetWorkingHours(hours: WorkingHours): Promise<Household> {
  return invoke<Household>("household_set_working_hours", { args: { hours } });
}
export async function householdSetDnd(windows: DndWindow[]): Promise<Household> {
  return invoke<Household>("household_set_dnd", { args: { windows } });
}
