import { invoke } from "@tauri-apps/api/core";

export interface Task {
  id: number;
  title: string;
  due_date: string | null;
  completed_at: number | null;
  created_at: number;
  proposal_id: number | null;
}

export interface Proposal {
  id: number;
  kind: string;
  rationale: string;
  diff: string;
  status: string;
  proposed_at: number;
  applied_at: number | null;
  skill: string;
}

export interface Event {
  id: number;
  title: string;
  start_at: number;
  end_at: number;
}

export async function listTasks(): Promise<Task[]> {
  return invoke<Task[]>("list_tasks");
}

export async function addTask(title: string, dueDate?: string | null): Promise<Task> {
  return invoke<Task>("add_task", { title, dueDate: dueDate ?? null });
}

export async function completeTask(id: number): Promise<void> {
  return invoke<void>("complete_task", { id });
}

export async function undoCompleteTask(id: number): Promise<void> {
  return invoke<void>("undo_complete_task", { id });
}

export async function updateTask(id: number, title: string): Promise<void> {
  return invoke<void>("update_task", { id, title });
}

export async function deleteTask(id: number): Promise<void> {
  return invoke<void>("delete_task", { id });
}

export async function listProposals(status?: string): Promise<Proposal[]> {
  return invoke<Proposal[]>("list_proposals", { status: status ?? null });
}

export async function approveProposal(id: number): Promise<Task[]> {
  return invoke<Task[]>("approve_proposal", { id });
}

export async function rejectProposal(id: number): Promise<void> {
  return invoke<void>("reject_proposal", { id });
}

export async function listEventsToday(): Promise<Event[]> {
  return invoke<Event[]>("list_events_today");
}
