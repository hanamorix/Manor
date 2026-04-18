//! Pure JSON-LD + LLM parsers for recipe import. No network, no file I/O.

use super::{IngredientLine, ImportMethod};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedRecipe {
    pub title: String,
    pub servings: Option<i32>,
    pub prep_time_mins: Option<i32>,
    pub cook_time_mins: Option<i32>,
    pub instructions: String,
    pub ingredients: Vec<IngredientLine>,
    pub source_url: String,
    pub source_host: String,
    pub import_method: ImportMethod,
    pub parse_notes: Vec<String>,
    pub hero_image_url: Option<String>,
}

/// Parse a recipe from schema.org JSON-LD embedded in HTML.
/// Returns None if no valid Recipe block is found.
pub fn parse_jsonld(html: &str) -> Option<ImportedRecipe> {
    let doc = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;

    for el in doc.select(&selector) {
        let raw = el.text().collect::<String>();
        let Ok(json) = serde_json::from_str::<Value>(&raw) else { continue };
        if let Some(recipe) = find_recipe_node(&json) {
            if let Some(r) = map_recipe_node(recipe) {
                return Some(r);
            }
        }
    }
    None
}

fn find_recipe_node(v: &Value) -> Option<&Value> {
    match v {
        Value::Object(_) => {
            if node_type_matches(v, "Recipe") { return Some(v); }
            if let Some(graph) = v.get("@graph").and_then(|g| g.as_array()) {
                for item in graph {
                    if node_type_matches(item, "Recipe") { return Some(item); }
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(find_recipe_node),
        _ => None,
    }
}

fn node_type_matches(node: &Value, wanted: &str) -> bool {
    match node.get("@type") {
        Some(Value::String(s)) => s == wanted,
        Some(Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(wanted)),
        _ => false,
    }
}

fn map_recipe_node(node: &Value) -> Option<ImportedRecipe> {
    let title = node.get("name").and_then(Value::as_str)?.trim().to_string();
    if title.is_empty() { return None; }

    let servings = parse_yield(node.get("recipeYield"));
    let prep_time_mins = parse_iso_duration_mins(node.get("prepTime"));
    let cook_time_mins = parse_iso_duration_mins(node.get("cookTime"));

    let ingredients = node.get("recipeIngredient")
        .and_then(Value::as_array)
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str())
            .map(split_ingredient_line)
            .collect::<Vec<_>>())
        .unwrap_or_default();

    let instructions = instructions_to_markdown(node.get("recipeInstructions"));
    let hero_image_url = extract_image(node.get("image"));

    Some(ImportedRecipe {
        title, servings, prep_time_mins, cook_time_mins,
        instructions, ingredients,
        source_url: String::new(),
        source_host: String::new(),
        import_method: ImportMethod::JsonLd,
        parse_notes: Vec::new(),
        hero_image_url,
    })
}

fn parse_yield(v: Option<&Value>) -> Option<i32> {
    match v? {
        Value::Number(n) => n.as_i64().map(|x| x as i32),
        Value::String(s) => {
            s.split(|c: char| !c.is_ascii_digit())
                .find(|s| !s.is_empty())
                .and_then(|t| t.parse().ok())
        }
        Value::Array(arr) => arr.iter().find_map(|v| parse_yield(Some(v))),
        _ => None,
    }
}

/// Parse ISO-8601 duration like "PT15M", "PT1H20M", "PT2H" → minutes.
fn parse_iso_duration_mins(v: Option<&Value>) -> Option<i32> {
    let s = v?.as_str()?;
    let s = s.strip_prefix("PT")?;
    let mut hours: i32 = 0;
    let mut mins: i32 = 0;
    let mut buf = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() { buf.push(ch); continue; }
        let n: i32 = buf.parse().unwrap_or(0);
        buf.clear();
        match ch {
            'H' => hours = n,
            'M' => mins = n,
            _ => {}
        }
    }
    Some(hours * 60 + mins)
}

fn split_ingredient_line(line: &str) -> IngredientLine {
    let line = line.trim();
    let qty_end = line.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '/' || *c == ' ' || *c == '½' || *c == '¼' || *c == '¾').count();
    let (qty_raw, rest) = line.split_at(qty_end);
    let qty = qty_raw.trim();

    const UNITS: &[&str] = &["tbsp","tsp","cup","cups","g","kg","ml","l","oz","lb","pcs","piece","pieces","clove","cloves","pinch","dash","handful","sprig","sprigs","can","cans","bunch","bunches"];
    let rest = rest.trim_start();
    let (unit, after_unit) = rest.split_once(' ').unwrap_or((rest, ""));
    let (quantity_text, name_plus) = if UNITS.iter().any(|u| unit.eq_ignore_ascii_case(u)) {
        let combined = if qty.is_empty() { unit.to_string() } else { format!("{} {}", qty, unit) };
        (if combined.is_empty() { None } else { Some(combined) }, after_unit.trim())
    } else {
        (if qty.is_empty() { None } else { Some(qty.to_string()) }, rest)
    };

    let (name, note) = match name_plus.split_once(',') {
        Some((n, note)) => (n.trim().to_string(), Some(note.trim().to_string())),
        None => (name_plus.trim().to_string(), None),
    };
    IngredientLine { quantity_text, ingredient_name: name, note }
}

fn instructions_to_markdown(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr.iter().enumerate().filter_map(|(i, step)| {
            let text = match step {
                Value::String(s) => s.clone(),
                Value::Object(_) => step.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                _ => return None,
            };
            if text.trim().is_empty() { None } else { Some(format!("{}. {}", i + 1, text.trim())) }
        }).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn extract_image(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Array(arr) => arr.iter().find_map(|v| extract_image(Some(v))),
        Value::Object(_) => v?.get("url").and_then(Value::as_str).map(String::from),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// LLM extraction fallback
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String>;
}

const LLM_PROMPT: &str = "You extract structured recipe data from webpage text. Output JSON with this exact shape:\n{\n  \"title\": str,\n  \"servings\": int|null,\n  \"prep_time_mins\": int|null,\n  \"cook_time_mins\": int|null,\n  \"instructions\": str (markdown, numbered steps),\n  \"ingredients\": [\n    {\"quantity_text\": str|null, \"ingredient_name\": str, \"note\": str|null}\n  ]\n}\nIf a field is not clearly stated, use null. Do not fabricate quantities.\nOutput ONLY the JSON, no prose before or after.\n\nWebpage content:\n";

#[derive(Deserialize)]
struct LlmRecipe {
    title: String,
    servings: Option<i32>,
    prep_time_mins: Option<i32>,
    cook_time_mins: Option<i32>,
    instructions: String,
    ingredients: Vec<IngredientLine>,
}

pub async fn extract_via_llm(
    page_text: &str,
    client: &dyn LlmClient,
    via_remote: bool,
) -> anyhow::Result<ImportedRecipe> {
    let truncated: String = page_text.chars().take(4096).collect();
    let prompt = format!("{}{}", LLM_PROMPT, truncated);

    let first = client.complete(&prompt).await?;
    let parsed: Result<LlmRecipe, _> = extract_json_block(&first);

    let llm_recipe = match parsed {
        Ok(r) => r,
        Err(_) => {
            let retry_prompt = format!("{}\n\n(Previous response was not valid JSON. Output ONLY JSON.)", prompt);
            let second = client.complete(&retry_prompt).await?;
            extract_json_block(&second)
                .map_err(|e| anyhow::anyhow!("failed to parse LLM JSON after retry: {}", e))?
        }
    };

    Ok(ImportedRecipe {
        title: llm_recipe.title,
        servings: llm_recipe.servings,
        prep_time_mins: llm_recipe.prep_time_mins,
        cook_time_mins: llm_recipe.cook_time_mins,
        instructions: llm_recipe.instructions,
        ingredients: llm_recipe.ingredients,
        source_url: String::new(),
        source_host: String::new(),
        import_method: if via_remote { ImportMethod::LlmRemote } else { ImportMethod::Llm },
        parse_notes: vec!["AI-extracted — please review quantities and steps.".into()],
        hero_image_url: None,
    })
}

fn extract_json_block<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T, serde_json::Error> {
    // Find first { and last } to be forgiving if the model prepends/appends prose.
    let start = s.find('{').unwrap_or(0);
    let end = s.rfind('}').map(|i| i + 1).unwrap_or(s.len());
    let slice = &s[start..end];
    serde_json::from_str::<T>(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bbc_good_food_jsonld() {
        let html = include_str!("../../tests/fixtures/recipe/bbc_good_food.html");
        let parsed = parse_jsonld(html).expect("parse succeeds");
        assert_eq!(parsed.title, "Thai green curry");
        assert_eq!(parsed.servings, Some(4));
        assert_eq!(parsed.prep_time_mins, Some(15));
        assert_eq!(parsed.cook_time_mins, Some(20));
        assert_eq!(parsed.ingredients.len(), 3);
        assert_eq!(parsed.ingredients[0].quantity_text.as_deref(), Some("1 tbsp"));
        assert_eq!(parsed.ingredients[0].ingredient_name, "vegetable oil");
        assert_eq!(parsed.hero_image_url.as_deref(), Some("https://bbcgoodfood.com/thai-curry.jpg"));
        assert!(parsed.instructions.contains("Heat the oil"));
    }

    #[test]
    fn parses_graph_wrapped_recipe() {
        let html = include_str!("../../tests/fixtures/recipe/nyt_cooking.html");
        let parsed = parse_jsonld(html).expect("found recipe in @graph");
        assert!(!parsed.title.is_empty());
        assert_eq!(parsed.title, "Carbonara");
    }

    #[test]
    fn parses_type_array() {
        let html = include_str!("../../tests/fixtures/recipe/serious_eats.html");
        assert!(parse_jsonld(html).is_some());
    }

    #[test]
    fn parses_numeric_yield() {
        let html = include_str!("../../tests/fixtures/recipe/allrecipes.html");
        let r = parse_jsonld(html).unwrap();
        assert_eq!(r.servings, Some(6));
    }

    #[test]
    fn handles_missing_prep_time() {
        let html = include_str!("../../tests/fixtures/recipe/bon_appetit.html");
        let r = parse_jsonld(html).unwrap();
        assert!(r.prep_time_mins.is_none());
    }

    #[test]
    fn parses_string_instructions() {
        let html = include_str!("../../tests/fixtures/recipe/delicious.html");
        let r = parse_jsonld(html).unwrap();
        assert!(!r.instructions.is_empty());
    }

    #[test]
    fn parses_image_object() {
        let html = include_str!("../../tests/fixtures/recipe/jamie_oliver.html");
        let r = parse_jsonld(html).unwrap();
        assert!(r.hero_image_url.is_some());
    }

    #[test]
    fn returns_none_without_jsonld() {
        let html = include_str!("../../tests/fixtures/recipe/ottolenghi.html");
        assert!(parse_jsonld(html).is_none());
    }
}

#[cfg(test)]
mod llm_tests {
    use super::*;

    struct StubLlm { response: String }
    #[async_trait::async_trait]
    impl LlmClient for StubLlm {
        async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn extracts_valid_json_via_llm() {
        let stub = StubLlm { response: r#"{
            "title":"Lentil dal","servings":2,"prep_time_mins":5,"cook_time_mins":25,
            "instructions":"1. Rinse lentils.\n2. Simmer.",
            "ingredients":[{"quantity_text":"200g","ingredient_name":"red lentils","note":null}]
        }"#.into() };
        let r = extract_via_llm("page text", &stub, false).await.unwrap();
        assert_eq!(r.title, "Lentil dal");
        assert_eq!(r.servings, Some(2));
        assert_eq!(r.ingredients.len(), 1);
        assert_eq!(r.import_method, ImportMethod::Llm);
    }

    #[tokio::test]
    async fn malformed_json_retries_then_errors() {
        struct BadLlm;
        #[async_trait::async_trait]
        impl LlmClient for BadLlm {
            async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
                Ok("not json".into())
            }
        }
        let err = extract_via_llm("x", &BadLlm, false).await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("parse"));
    }
}
