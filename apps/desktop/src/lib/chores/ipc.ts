import { invoke } from "@tauri-apps/api/core";

export type RotationKind = "round_robin" | "least_completed" | "fixed" | "none";

export interface Chore {
  id: number;
  title: string;
  emoji: string;
  rrule: string;
  next_due: number;
  rotation: RotationKind;
  active: boolean;
  created_at: number;
  deleted_at: number | null;
}

export interface ChoreCompletion {
  id: number;
  chore_id: number;
  completed_at: number;
  completed_by: number | null;
  created_at: number;
}

export interface RotationMember {
  id: number;
  chore_id: number;
  person_id: number;
  position: number;
  current: boolean;
}

export interface FairnessNudge {
  chore_id: number;
  chore_title: string;
  person_id: number;
  person_name: string;
  days_ago: number;
}

export async function listChoresDueToday(): Promise<Chore[]> {
  return invoke<Chore[]>("list_chores_due_today");
}

export async function listAllChores(): Promise<Chore[]> {
  return invoke<Chore[]>("list_all_chores");
}

export async function createChore(args: {
  title: string;
  emoji: string;
  rrule: string;
  firstDue: number;
  rotation: RotationKind;
}): Promise<Chore> {
  return invoke<Chore>("create_chore", { args });
}

export async function updateChore(args: {
  id: number;
  title: string;
  emoji: string;
  rrule: string;
  rotation: RotationKind;
}): Promise<Chore> {
  return invoke<Chore>("update_chore", { args });
}

export async function deleteChore(id: number): Promise<void> {
  return invoke<void>("delete_chore", { id });
}

export async function completeChore(id: number, completedBy: number | null = null): Promise<Chore> {
  return invoke<Chore>("complete_chore", { args: { id, completedBy } });
}

export async function skipChore(id: number): Promise<Chore> {
  return invoke<Chore>("skip_chore", { id });
}

export async function listChoreCompletions(choreId: number, limit: number = 20): Promise<ChoreCompletion[]> {
  return invoke<ChoreCompletion[]>("list_chore_completions", { choreId, limit });
}

export async function listChoreRotation(choreId: number): Promise<RotationMember[]> {
  return invoke<RotationMember[]>("list_chore_rotation", { choreId });
}

export async function checkChoreFairness(): Promise<FairnessNudge[]> {
  return invoke<FairnessNudge[]>("check_chore_fairness");
}

export async function addPerson(name: string): Promise<number> {
  return invoke<number>("add_person", { name });
}
