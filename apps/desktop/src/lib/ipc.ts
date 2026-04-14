import { invoke } from "@tauri-apps/api/core";

export interface PingResponse {
  message: string;
  core_version: string;
}

/**
 * Smoke-test IPC call: should return {"pong", core_version}.
 */
export async function ping(): Promise<PingResponse> {
  return await invoke<PingResponse>("ping");
}
