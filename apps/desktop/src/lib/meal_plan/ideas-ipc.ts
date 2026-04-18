import { invoke } from "@tauri-apps/api/core";
import type { Recipe, ImportPreview } from "../recipe/recipe-ipc";

export type { ImportPreview };

export interface IdeaTitle {
  title: string;
  blurb: string;
}

export async function librarySample(): Promise<Recipe[]> {
  return await invoke<Recipe[]>("meal_ideas_library_sample");
}

export async function llmTitles(): Promise<IdeaTitle[]> {
  return await invoke<IdeaTitle[]>("meal_ideas_llm_titles");
}

export async function llmExpand(title: string, blurb: string): Promise<ImportPreview> {
  return await invoke<ImportPreview>("meal_ideas_llm_expand", { title, blurb });
}
