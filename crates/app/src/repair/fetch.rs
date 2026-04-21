//! Fetch + readability trim (L4d).

use anyhow::Result;
use scraper::{Html, Selector};

pub const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
pub const FETCH_TIMEOUT_SECS: u64 = 10;
pub const TRIMMED_TEXT_CAP_BYTES: usize = 2 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("couldn't reach {0}")]
    FetchFailed(String),
    #[error("{0} isn't html (content-type: {1})")]
    NotHtml(String, String),
    #[error("{0} is too large")]
    TooLarge(String),
}

pub async fn fetch_and_trim(client: &reqwest::Client, url: &str) -> Result<String> {
    let vetted = manor_core::net::ssrf::vet_url(url)
        .map_err(|e| anyhow::anyhow!("URL rejected: {e}"))?;
    fetch_and_trim_inner(client, vetted).await
}

/// Inner fetch logic that operates on an already-vetted URL.
/// Callers (including tests) that have already validated the URL may call this directly.
async fn fetch_and_trim_inner(client: &reqwest::Client, vetted: url::Url) -> Result<String> {
    let url_str = vetted.as_str().to_string();
    let mut resp = client
        .get(vetted)
        .send()
        .await
        .map_err(|_| FetchError::FetchFailed(url_str.clone()))?;

    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ctype.contains("text/html") {
        return Err(FetchError::NotHtml(url_str.clone(), ctype).into());
    }
    // Stream body with cap — do NOT trust Content-Length header.
    let mut body_bytes = Vec::<u8>::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|_| FetchError::FetchFailed(url_str.clone()))?
    {
        if body_bytes.len() + chunk.len() > MAX_BODY_BYTES as usize {
            return Err(FetchError::TooLarge(url_str).into());
        }
        body_bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    Ok(trim_html_to_excerpt(&body))
}

pub fn trim_html_to_excerpt(body: &str) -> String {
    let doc = Html::parse_document(body);
    let strip_selectors = [
        "script", "style", "nav", "header", "footer", "aside", "noscript", "form", "iframe", "svg",
    ];
    let preferred = [
        Selector::parse("main").unwrap(),
        Selector::parse("article").unwrap(),
        Selector::parse("[role=\"main\"]").unwrap(),
    ];
    let mut text = String::new();
    for sel in &preferred {
        if let Some(el) = doc.select(sel).next() {
            collect_text_excluding(&el, &strip_selectors, &mut text);
            if !text.trim().is_empty() {
                break;
            }
        }
    }
    if text.trim().is_empty() {
        if let Some(body_el) = doc.select(&Selector::parse("body").unwrap()).next() {
            collect_text_excluding(&body_el, &strip_selectors, &mut text);
        }
    }
    let collapsed = collapse_whitespace(&text);
    truncate_to_byte_budget(&collapsed, TRIMMED_TEXT_CAP_BYTES)
}

fn collect_text_excluding(root: &scraper::ElementRef<'_>, skip_tags: &[&str], out: &mut String) {
    for node in root.descendants() {
        if let scraper::node::Node::Text(t) = node.value() {
            let has_skipped_ancestor = node.ancestors().any(|a| {
                if let scraper::node::Node::Element(el) = a.value() {
                    skip_tags
                        .iter()
                        .any(|tag| tag.eq_ignore_ascii_case(el.name()))
                } else {
                    false
                }
            });
            if !has_skipped_ancestor {
                out.push_str(t);
                out.push(' ');
            }
        }
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn truncate_to_byte_budget(s: &str, budget: usize) -> String {
    if s.len() <= budget {
        return s.to_string();
    }
    let mut cut = budget;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s[..cut].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fetch_rejects_loopback() {
        let client = reqwest::Client::new();
        let err = fetch_and_trim(&client, "http://127.0.0.1/").await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("URL") || msg.contains("private") || msg.contains("scheme") || msg.contains("rejected"),
            "got: {msg}",
        );
    }

    #[tokio::test]
    async fn fetch_rejects_file_scheme() {
        let client = reqwest::Client::new();
        let err = fetch_and_trim(&client, "file:///etc/passwd").await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("URL") || msg.contains("scheme") || msg.contains("rejected"),
            "got: {msg}",
        );
    }

    #[test]
    fn trim_strips_nav_script_style() {
        let html = r##"
        <html>
          <head><style>body{color:red}</style></head>
          <body>
            <nav>Home | About | Contact</nav>
            <script>alert("hi");</script>
            <main>
              <h1>Real Content</h1>
              <p>Keep this paragraph.</p>
            </main>
            <footer>copyright 2025</footer>
          </body>
        </html>
        "##;
        let out = trim_html_to_excerpt(html);
        assert!(out.contains("Real Content"));
        assert!(out.contains("Keep this paragraph"));
        assert!(!out.contains("Home | About"));
        assert!(!out.contains("alert"));
        assert!(!out.contains("copyright"));
    }

    #[test]
    fn trim_prefers_main_over_body() {
        let html = r##"
        <html><body>
          <div>Sidebar noise</div>
          <main><p>Important main content.</p></main>
          <div>Footer noise</div>
        </body></html>
        "##;
        let out = trim_html_to_excerpt(html);
        assert!(out.contains("Important main content"));
        assert!(!out.contains("Sidebar noise"));
        assert!(!out.contains("Footer noise"));
    }

    #[test]
    fn trim_falls_back_to_body_when_no_main() {
        let html = "<html><body><p>Just a body paragraph.</p></body></html>";
        let out = trim_html_to_excerpt(html);
        assert!(out.contains("Just a body paragraph"));
    }

    #[test]
    fn trim_caps_at_2kb() {
        let big = "lorem ipsum ".repeat(500);
        let html = format!("<html><body><main>{}</main></body></html>", big);
        let out = trim_html_to_excerpt(&html);
        assert!(out.len() <= TRIMMED_TEXT_CAP_BYTES);
    }

    #[test]
    fn trim_collapses_whitespace() {
        let html = "<html><body><main>Hello\n\n\n    World  </main></body></html>";
        let out = trim_html_to_excerpt(html);
        assert_eq!(out, "Hello World");
    }

    #[tokio::test]
    async fn fetch_rejects_non_html_content_type() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("{}"),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        // Use fetch_and_trim_inner directly: wiremock binds to 127.0.0.1 which the SSRF
        // guard correctly blocks. This test covers content-type checking, not SSRF.
        let vetted = url::Url::parse(&server.uri()).unwrap();
        let err = fetch_and_trim_inner(&client, vetted)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't html"), "got: {}", err);
    }

    #[tokio::test]
    async fn fetch_succeeds_on_html_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        let body = b"<html><body><main>From wiremock</main></body></html>".to_vec();
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html; charset=utf-8")
                    .set_body_bytes(body),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        // Use fetch_and_trim_inner directly: wiremock binds to 127.0.0.1 which the SSRF
        // guard correctly blocks. This test covers HTML-trimming, not SSRF.
        let vetted = url::Url::parse(&server.uri()).unwrap();
        let text = fetch_and_trim_inner(&client, vetted).await.unwrap();
        assert_eq!(text, "From wiremock");
    }
}
