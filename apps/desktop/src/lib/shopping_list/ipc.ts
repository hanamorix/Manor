import { invoke } from "@tauri-apps/api/core";

export type ItemSource = "generated" | "manual";

export interface ShoppingListItem {
  id: string;
  ingredient_name: string;
  quantity_text: string | null;
  note: string | null;
  recipe_id: string | null;
  recipe_title: string | null;
  source: ItemSource;
  position: number;
  ticked: boolean;
  created_at: number;
  updated_at: number;
}

export interface GeneratedReport {
  items_added: number;
  items_skipped_staple: number;
  ghost_recipes_skipped: number;
}

export async function list(): Promise<ShoppingListItem[]> {
  return await invoke<ShoppingListItem[]>("shopping_list_list");
}

export async function addManual(ingredientName: string): Promise<string> {
  return await invoke<string>("shopping_list_add_manual", { ingredientName });
}

export async function toggle(id: string): Promise<void> {
  await invoke("shopping_list_toggle", { id });
}

export async function deleteItem(id: string): Promise<void> {
  await invoke("shopping_list_delete", { id });
}

export async function regenerate(weekStart: string): Promise<GeneratedReport> {
  return await invoke<GeneratedReport>("shopping_list_regenerate", { weekStart });
}
