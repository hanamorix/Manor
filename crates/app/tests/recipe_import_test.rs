use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const JSONLD_PAGE: &str = r#"<!DOCTYPE html><html><head>
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"Recipe","name":"Tomato soup",
 "recipeYield":"2","prepTime":"PT5M","cookTime":"PT20M",
 "recipeIngredient":["500g tomatoes","1 onion"],
 "recipeInstructions":[{"@type":"HowToStep","text":"Chop."},{"@type":"HowToStep","text":"Simmer."}]}
</script></head><body></body></html>"#;

#[tokio::test]
async fn preview_succeeds_via_jsonld() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/tomato"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_raw(JSONLD_PAGE, "text/html; charset=utf-8"))
        .mount(&server).await;

    let url = format!("{}/tomato", server.uri());
    let preview = manor_app::recipe::importer::preview_inner(url::Url::parse(&url).unwrap(), None).await
        .expect("preview succeeds");
    assert_eq!(preview.recipe_draft.title, "Tomato soup");
    assert_eq!(preview.import_method, manor_core::recipe::ImportMethod::JsonLd);
    assert_eq!(preview.recipe_draft.ingredients.len(), 2);
}

#[tokio::test]
async fn fails_on_non_html() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pdf"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_raw(&b"%PDF-1.4"[..], "application/pdf"))
        .mount(&server).await;

    let url = format!("{}/pdf", server.uri());
    let err = manor_app::recipe::importer::preview_inner(url::Url::parse(&url).unwrap(), None).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("not html") || err.to_string().to_lowercase().contains("html"));
}

use async_trait::async_trait;
use manor_core::recipe::import::LlmClient;

struct StubLlm { response: String }

#[async_trait]
impl LlmClient for StubLlm {
    async fn complete(&self, _prompt: &str) -> anyhow::Result<String> { Ok(self.response.clone()) }
}

#[tokio::test]
async fn falls_back_to_llm_when_no_jsonld() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/blog-post"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_raw("<html><body>Cook onions and eat. Serves 4.</body></html>", "text/html"))
        .mount(&server).await;

    let stub = StubLlm { response: r#"{"title":"Onion dinner","servings":4,"prep_time_mins":null,"cook_time_mins":null,"instructions":"1. Cook.","ingredients":[{"quantity_text":null,"ingredient_name":"onions","note":null}]}"#.into() };

    let url = format!("{}/blog-post", server.uri());
    let preview = manor_app::recipe::importer::preview_inner(
        url::Url::parse(&url).unwrap(),
        Some(&stub),
    )
    .await
    .unwrap();
    assert_eq!(preview.recipe_draft.title, "Onion dinner");
    assert_eq!(preview.import_method, manor_core::recipe::ImportMethod::Llm);
}
