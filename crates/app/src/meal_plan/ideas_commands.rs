//! Meal ideas Tauri commands — library sample + LLM titles + LLM expand.

use crate::assistant::commands::Db;
use crate::assistant::ollama::{resolve_model, OllamaClient, DEFAULT_ENDPOINT};
use crate::recipe::llm_adapter::OllamaLlmAdapter;
use crate::recipe::importer::ImportPreview;
use manor_core::meal_plan::ideas::library_ranked;
use manor_core::recipe::import::{extract_json_array_block_public, extract_json_block_public, ImportedRecipe, LlmClient};
use manor_core::recipe::{ImportMethod, Recipe, RecipeDraft};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaTitle {
    pub title: String,
    pub blurb: String,
}

#[tauri::command]
pub fn meal_ideas_library_sample(state: State<'_, Db>) -> Result<Vec<Recipe>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let ranked = library_ranked(&conn).map_err(|e| e.to_string())?;

    let top_n = 10.min(ranked.len());
    let mut pool: Vec<Recipe> = ranked.into_iter().take(top_n).map(|s| s.recipe).collect();
    pool.shuffle(&mut rand::thread_rng());
    pool.truncate(3);
    Ok(pool)
}

const TITLES_PROMPT: &str = "You suggest 3 home-cookable dinner recipes. Output JSON exactly:\n[\n  {\"title\": str, \"blurb\": str (one sentence, <100 chars, includes timing hint)},\n  {\"title\": str, \"blurb\": str},\n  {\"title\": str, \"blurb\": str}\n]\nVary cuisines. Prefer weeknight-accessible ingredients. No prose before or after the JSON.";

#[tauri::command]
pub async fn meal_ideas_llm_titles(state: State<'_, Db>) -> Result<Vec<IdeaTitle>, String> {
    let model = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        resolve_model(&conn)
    };
    let adapter = OllamaLlmAdapter(OllamaClient::new(DEFAULT_ENDPOINT, &model));
    run_titles(&adapter).await
}

async fn run_titles(client: &dyn LlmClient) -> Result<Vec<IdeaTitle>, String> {
    let first = client.complete(TITLES_PROMPT).await
        .map_err(|_| "AI unavailable — try again later or check Settings → AI.".to_string())?;
    let parsed: Result<Vec<IdeaTitle>, _> = extract_json_array_block_public::<Vec<IdeaTitle>>(&first);

    let titles = match parsed {
        Ok(t) => t,
        Err(_) => {
            let retry = format!("{}\n\n(Previous response was not valid JSON. Output ONLY JSON.)", TITLES_PROMPT);
            let second = client.complete(&retry).await
                .map_err(|_| "AI unavailable — try again later or check Settings → AI.".to_string())?;
            extract_json_array_block_public::<Vec<IdeaTitle>>(&second)
                .map_err(|_| "AI returned invalid response — try again.".to_string())?
        }
    };

    if titles.is_empty() {
        return Err("AI returned no suggestions — try again.".to_string());
    }
    Ok(titles.into_iter().take(3).collect())
}

const EXPAND_PROMPT_PREFIX: &str = "You extract structured recipe data from a recipe description. Output JSON with this exact shape:\n{\n  \"title\": str,\n  \"servings\": int|null,\n  \"prep_time_mins\": int|null,\n  \"cook_time_mins\": int|null,\n  \"instructions\": str (markdown, numbered steps),\n  \"ingredients\": [\n    {\"quantity_text\": str|null, \"ingredient_name\": str, \"note\": str|null}\n  ]\n}\nIf a field is not clearly stated, use reasonable defaults for a 2-serving weeknight meal. You may invent plausible ingredient quantities.\nOutput ONLY the JSON.\n\nRecipe description:\n";

#[tauri::command]
pub async fn meal_ideas_llm_expand(state: State<'_, Db>, title: String, blurb: String) -> Result<ImportPreview, String> {
    let model = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        resolve_model(&conn)
    };
    let adapter = OllamaLlmAdapter(OllamaClient::new(DEFAULT_ENDPOINT, &model));
    run_expand(&adapter, &title, &blurb).await
}

async fn run_expand(client: &dyn LlmClient, title: &str, blurb: &str) -> Result<ImportPreview, String> {
    let prompt = format!("{}Title: {}\nSummary: {}", EXPAND_PROMPT_PREFIX, title, blurb);
    let raw = client.complete(&prompt).await
        .map_err(|_| "AI unavailable — try again later.".to_string())?;

    let parsed: Result<ExpandShape, _> = extract_json_block_public(&raw);
    let llm_recipe = match parsed {
        Ok(r) => r,
        Err(_) => {
            let retry = format!("{}\n\n(Previous response was not valid JSON. Output ONLY JSON.)", prompt);
            let second = client.complete(&retry).await
                .map_err(|_| "AI unavailable — try again later.".to_string())?;
            extract_json_block_public::<ExpandShape>(&second)
                .map_err(|_| "AI returned invalid recipe — try again.".to_string())?
        }
    };

    // Build ImportedRecipe, then ImportPreview.
    let imp = ImportedRecipe {
        title: llm_recipe.title,
        servings: llm_recipe.servings,
        prep_time_mins: llm_recipe.prep_time_mins,
        cook_time_mins: llm_recipe.cook_time_mins,
        instructions: llm_recipe.instructions,
        ingredients: llm_recipe.ingredients,
        source_url: String::new(),
        source_host: String::new(),
        import_method: ImportMethod::Llm,
        parse_notes: vec!["AI-extracted — please review quantities and steps.".into()],
        hero_image_url: None,
    };
    Ok(to_preview_from_imported(imp))
}

#[derive(Deserialize)]
struct ExpandShape {
    title: String,
    servings: Option<i32>,
    prep_time_mins: Option<i32>,
    cook_time_mins: Option<i32>,
    instructions: String,
    ingredients: Vec<manor_core::recipe::IngredientLine>,
}

fn to_preview_from_imported(imp: ImportedRecipe) -> ImportPreview {
    let notes = imp.parse_notes.clone();
    let method = imp.import_method.clone();
    let draft = RecipeDraft {
        title: imp.title,
        servings: imp.servings,
        prep_time_mins: imp.prep_time_mins,
        cook_time_mins: imp.cook_time_mins,
        instructions: imp.instructions,
        source_url: None,
        source_host: None,
        import_method: imp.import_method,
        hero_attachment_uuid: None,
        ingredients: imp.ingredients,
    };
    ImportPreview {
        recipe_draft: draft,
        import_method: method,
        parse_notes: notes,
        hero_image_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct StubLlm { responses: std::sync::Mutex<Vec<String>> }

    #[async_trait]
    impl LlmClient for StubLlm {
        async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
            let mut r = self.responses.lock().unwrap();
            Ok(r.remove(0))
        }
    }

    fn stub(resps: &[&str]) -> StubLlm {
        StubLlm { responses: std::sync::Mutex::new(resps.iter().map(|s| s.to_string()).collect()) }
    }

    #[tokio::test]
    async fn titles_happy_path() {
        let s = stub(&[r#"[{"title":"A","blurb":"x"},{"title":"B","blurb":"y"},{"title":"C","blurb":"z"}]"#]);
        let titles = run_titles(&s).await.unwrap();
        assert_eq!(titles.len(), 3);
        assert_eq!(titles[0].title, "A");
    }

    #[tokio::test]
    async fn titles_malformed_then_retry() {
        let s = stub(&[
            "not json",
            r#"[{"title":"X","blurb":"x"}]"#,
        ]);
        let titles = run_titles(&s).await.unwrap();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].title, "X");
    }

    #[tokio::test]
    async fn titles_both_fail_returns_err() {
        let s = stub(&["not json", "still not json"]);
        let err = run_titles(&s).await.unwrap_err();
        assert!(err.to_lowercase().contains("invalid") || err.to_lowercase().contains("unavailable"));
    }

    #[tokio::test]
    async fn expand_happy_path() {
        let s = stub(&[r#"{"title":"Miso","servings":2,"prep_time_mins":5,"cook_time_mins":25,"instructions":"1. Cook.","ingredients":[{"quantity_text":null,"ingredient_name":"aubergine","note":null}]}"#]);
        let preview = run_expand(&s, "Miso", "blurb").await.unwrap();
        assert_eq!(preview.recipe_draft.title, "Miso");
        assert_eq!(preview.import_method, ImportMethod::Llm);
        assert!(preview.recipe_draft.source_url.is_none());
    }
}
