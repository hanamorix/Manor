import { invoke } from "@tauri-apps/api/core";
import type { Recipe } from "../recipe/recipe-ipc";

export interface MealPlanEntryWithRecipe {
  entry_date: string;
  recipe: Recipe | null;
}

export async function weekGet(startDate: string): Promise<MealPlanEntryWithRecipe[]> {
  return await invoke<MealPlanEntryWithRecipe[]>("meal_plan_week_get", { startDate });
}

export async function todayGet(): Promise<MealPlanEntryWithRecipe | null> {
  return await invoke<MealPlanEntryWithRecipe | null>("meal_plan_today_get");
}

export async function setEntry(date: string, recipeId: string): Promise<void> {
  await invoke("meal_plan_set_entry", { date, recipeId });
}

export async function clearEntry(date: string): Promise<void> {
  await invoke("meal_plan_clear_entry", { date });
}
