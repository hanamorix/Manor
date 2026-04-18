import { invoke } from "@tauri-apps/api/core";

export type ImportMethod = "manual" | "jsonld" | "llm" | "llm_remote";

export interface IngredientLine {
  quantity_text: string | null;
  ingredient_name: string;
  note: string | null;
}

export interface Recipe {
  id: string;
  title: string;
  servings: number | null;
  prep_time_mins: number | null;
  cook_time_mins: number | null;
  instructions: string;
  source_url: string | null;
  source_host: string | null;
  import_method: ImportMethod;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
  ingredients: IngredientLine[];
}

export interface RecipeDraft {
  title: string;
  servings: number | null;
  prep_time_mins: number | null;
  cook_time_mins: number | null;
  instructions: string;
  source_url: string | null;
  source_host: string | null;
  import_method: ImportMethod;
  ingredients: IngredientLine[];
}

export interface ImportPreview {
  recipe_draft: RecipeDraft;
  import_method: ImportMethod;
  parse_notes: string[];
  hero_image_url: string | null;
}

export async function list(search?: string, tagIds: string[] = []): Promise<Recipe[]> {
  return await invoke<Recipe[]>("recipe_list", { args: { search, tag_ids: tagIds } });
}

export async function get(id: string): Promise<Recipe | null> {
  return await invoke<Recipe | null>("recipe_get", { id });
}

export async function create(draft: RecipeDraft): Promise<string> {
  return await invoke<string>("recipe_create", { draft });
}

export async function update(id: string, draft: RecipeDraft): Promise<void> {
  await invoke("recipe_update", { id, draft });
}

export async function deleteRecipe(id: string): Promise<void> {
  await invoke("recipe_delete", { id });
}

export async function restore(id: string): Promise<void> {
  await invoke("recipe_restore", { id });
}

export async function importPreview(url: string): Promise<ImportPreview> {
  return await invoke<ImportPreview>("recipe_import_preview", { url });
}

export async function importCommit(draft: RecipeDraft, heroImageUrl: string | null): Promise<string> {
  return await invoke<string>("recipe_import_commit", { args: { draft, hero_image_url: heroImageUrl } });
}
