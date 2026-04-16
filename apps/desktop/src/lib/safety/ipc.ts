import { invoke } from "@tauri-apps/api/core";

// ── Trash ────────────────────────────────────────────────────────────────────

export interface TrashEntry {
  entity_type: string;
  entity_id: number;
  title: string;
  deleted_at: number;
}

export async function trashList(): Promise<TrashEntry[]> {
  return invoke<TrashEntry[]>("trash_list");
}
export async function trashRestore(entity_type: string, entity_id: number): Promise<void> {
  return invoke<void>("trash_restore", { entityType: entity_type, entityId: entity_id });
}
export async function trashPermanentDelete(entity_type: string, entity_id: number): Promise<void> {
  return invoke<void>("trash_permanent_delete", { entityType: entity_type, entityId: entity_id });
}
export async function trashEmptyAll(): Promise<Array<[string, number]>> {
  return invoke<Array<[string, number]>>("trash_empty_all");
}

// ── Backup ───────────────────────────────────────────────────────────────────

export interface BackupEntry {
  path: string;
  mtime: number;
  size_bytes: number;
}

export async function backupSetPassphrase(passphrase: string): Promise<void> {
  return invoke<void>("backup_set_passphrase", { passphrase });
}
export async function backupHasPassphrase(): Promise<boolean> {
  return invoke<boolean>("backup_has_passphrase");
}
export async function backupCreateNow(outDir: string): Promise<string> {
  return invoke<string>("backup_create_now", { outDir });
}
export async function backupList(dir: string): Promise<BackupEntry[]> {
  return invoke<BackupEntry[]>("backup_list", { dir });
}
export async function backupRestore(
  backupPath: string,
  providedPassphrase: string,
): Promise<string> {
  return invoke<string>("backup_restore", { backupPath, providedPassphrase });
}

export interface ScheduleArgs {
  programPath: string;
  outDir: string;
  weekday: number; // 0 Sun – 6 Sat
  hour: number;
  minute: number;
}

export async function backupScheduleInstall(args: ScheduleArgs): Promise<void> {
  return invoke<void>("backup_schedule_install", { args });
}
export async function backupScheduleUninstall(): Promise<void> {
  return invoke<void>("backup_schedule_uninstall");
}
export async function backupScheduleIsInstalled(): Promise<boolean> {
  return invoke<boolean>("backup_schedule_is_installed");
}

// ── Panic ────────────────────────────────────────────────────────────────────

export async function panicEraseEverything(confirmation: string): Promise<void> {
  return invoke<void>("panic_erase_everything", { confirmation });
}
