import { invoke } from "@tauri-apps/api/core";

export interface MaintenanceSchedule {
  id: string;
  asset_id: string;
  task: string;
  interval_months: number;
  last_done_date: string | null;
  next_due_date: string;
  notes: string;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface MaintenanceScheduleDraft {
  asset_id: string;
  task: string;
  interval_months: number;
  last_done_date: string | null;
  notes: string;
}

export interface ScheduleWithAsset {
  schedule: MaintenanceSchedule;
  asset_name: string;
  asset_category: string;
}

export async function listForAsset(assetId: string): Promise<MaintenanceSchedule[]> {
  return await invoke<MaintenanceSchedule[]>("maintenance_schedule_list_for_asset", { assetId });
}

export async function get(id: string): Promise<MaintenanceSchedule | null> {
  return await invoke<MaintenanceSchedule | null>("maintenance_schedule_get", { id });
}

export async function create(draft: MaintenanceScheduleDraft): Promise<string> {
  return await invoke<string>("maintenance_schedule_create", { draft });
}

export async function update(id: string, draft: MaintenanceScheduleDraft): Promise<void> {
  await invoke("maintenance_schedule_update", { id, draft });
}

export async function markDone(id: string): Promise<void> {
  await invoke("maintenance_schedule_mark_done", { id });
}

export async function deleteSchedule(id: string): Promise<void> {
  await invoke("maintenance_schedule_delete", { id });
}

export async function dueSoon(): Promise<ScheduleWithAsset[]> {
  return await invoke<ScheduleWithAsset[]>("maintenance_due_soon");
}

export async function dueTodayAndOverdue(): Promise<ScheduleWithAsset[]> {
  return await invoke<ScheduleWithAsset[]>("maintenance_due_today_and_overdue");
}

export async function overdueCount(): Promise<number> {
  return await invoke<number>("maintenance_overdue_count");
}
