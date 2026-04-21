//! DuckDuckGo + YouTube URL scrapers (L4d).
//!
//! Both return `Ok(vec![])` on parse mismatch (empty results / unparseable HTML).
//! HTTP failures bubble up as `Err`, except YouTube (sidecar) which swallows
//! network errors and returns `Ok(vec![])` to keep the pipeline resilient.

use anyhow::{Context, Result};
use manor_core::repair::RepairSource;
use scraper::{Html, Selector};

pub async fn duckduckgo_top_n(
    client: &reqwest::Client,
    query: &str,
    n: usize,
) -> Result<Vec<RepairSource>> {
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );
    let vetted = manor_core::net::ssrf::vet_url(&url)
        .map_err(|e| anyhow::anyhow!("url rejected: {e}"))?;
    let resp = client
        .get(vetted)
        .send()
        .await
        .context("duckduckgo request failed")?;
    let body = resp.text().await.context("duckduckgo body read failed")?;
    Ok(parse_ddg_html(&body, n))
}

fn parse_ddg_html(body: &str, n: usize) -> Vec<RepairSource> {
    let doc = Html::parse_document(body);
    let title_selector = Selector::parse(".result__title a").unwrap();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for a in doc.select(&title_selector) {
        if out.len() >= n {
            break;
        }
        let href = match a.value().attr("href") {
            Some(h) => h,
            None => continue,
        };
        let url = unwrap_ddg_redirector(href).unwrap_or_else(|| href.to_string());
        if seen.contains(&url) {
            continue;
        }
        let title = a.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }
        seen.insert(url.clone());
        out.push(RepairSource { url, title });
    }
    out
}

fn unwrap_ddg_redirector(href: &str) -> Option<String> {
    // /l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2F → https://example.com/
    let needle = "uddg=";
    let idx = href.find(needle)?;
    let encoded = &href[idx + needle.len()..];
    let end = encoded.find('&').unwrap_or(encoded.len());
    let encoded = &encoded[..end];
    urlencoding::decode(encoded).ok().map(|s| s.into_owned())
}

pub async fn youtube_top_n(
    client: &reqwest::Client,
    query: &str,
    n: usize,
) -> Result<Vec<RepairSource>> {
    let url = format!(
        "https://www.youtube.com/results?search_query={}",
        urlencoding::encode(query)
    );
    let vetted = match manor_core::net::ssrf::vet_url(&url) {
        Ok(u) => u,
        Err(_) => return Ok(Vec::new()), // sidecar — never fail pipeline
    };
    let resp = match client.get(vetted).send().await {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()), // sidecar — never fail pipeline
    };
    let body = match resp.text().await {
        Ok(b) => b,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(parse_youtube_html(&body, n))
}

fn parse_youtube_html(body: &str, n: usize) -> Vec<RepairSource> {
    let Some(start) = body.find("var ytInitialData = ") else {
        return Vec::new();
    };
    let after = &body[start + "var ytInitialData = ".len()..];
    let Some(end) = after.find("};") else {
        return Vec::new();
    };
    let json_str = &after[..=end]; // include the '}'
    let Ok(root) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return Vec::new();
    };
    let items = root
        .pointer("/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents/0/itemSectionRenderer/contents")
        .and_then(|v| v.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        if out.len() >= n {
            break;
        }
        let Some(renderer) = item.pointer("/videoRenderer") else {
            continue;
        };
        let Some(video_id) = renderer.pointer("/videoId").and_then(|v| v.as_str()) else {
            continue;
        };
        let title = renderer
            .pointer("/title/runs/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if title.is_empty() {
            continue;
        }
        out.push(RepairSource {
            url: format!("https://www.youtube.com/watch?v={}", video_id),
            title,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ddg_extracts_top_n_titles_and_hrefs() {
        let html = r##"
        <html><body>
          <div class="result">
            <h2 class="result__title"><a href="/l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2Fa">Result A</a></h2>
          </div>
          <div class="result">
            <h2 class="result__title"><a href="https://example.com/b">Result B</a></h2>
          </div>
          <div class="result">
            <h2 class="result__title"><a href="https://example.com/c">Result C</a></h2>
          </div>
          <div class="result">
            <h2 class="result__title"><a href="https://example.com/d">Result D</a></h2>
          </div>
        </body></html>
        "##;
        let out = parse_ddg_html(html, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].url, "https://example.com/a");
        assert_eq!(out[0].title, "Result A");
        assert_eq!(out[1].url, "https://example.com/b");
        assert_eq!(out[2].url, "https://example.com/c");
    }

    #[test]
    fn parse_ddg_dedupes_repeated_hrefs() {
        let html = r##"
        <html><body>
          <div class="result"><h2 class="result__title"><a href="https://example.com/a">Result A</a></h2></div>
          <div class="result"><h2 class="result__title"><a href="https://example.com/a">Result A</a></h2></div>
          <div class="result"><h2 class="result__title"><a href="https://example.com/b">Result B</a></h2></div>
        </body></html>
        "##;
        let out = parse_ddg_html(html, 3);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://example.com/a");
        assert_eq!(out[1].url, "https://example.com/b");
    }

    #[test]
    fn parse_ddg_returns_empty_on_no_matches() {
        let html = "<html><body><p>no results</p></body></html>";
        let out = parse_ddg_html(html, 3);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_youtube_extracts_video_ids_and_titles() {
        let html = r##"
        <html><body>
        <script>var ytInitialData = {
          "contents":{
            "twoColumnSearchResultsRenderer":{
              "primaryContents":{
                "sectionListRenderer":{
                  "contents":[{
                    "itemSectionRenderer":{
                      "contents":[
                        {"videoRenderer":{"videoId":"abcDEF123","title":{"runs":[{"text":"Fix Your Boiler"}]}}},
                        {"videoRenderer":{"videoId":"xyzUVW456","title":{"runs":[{"text":"Boiler Teardown"}]}}}
                      ]
                    }
                  }]
                }
              }
            }
          }
        };</script>
        </body></html>
        "##;
        let out = parse_youtube_html(html, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://www.youtube.com/watch?v=abcDEF123");
        assert_eq!(out[0].title, "Fix Your Boiler");
        assert_eq!(out[1].url, "https://www.youtube.com/watch?v=xyzUVW456");
    }

    #[test]
    fn parse_youtube_returns_empty_on_missing_initial_data() {
        let html = "<html><body>nothing here</body></html>";
        let out = parse_youtube_html(html, 2);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_youtube_returns_empty_on_malformed_json() {
        let html = r##"<script>var ytInitialData = {not json};</script>"##;
        let out = parse_youtube_html(html, 2);
        assert!(out.is_empty());
    }
}
