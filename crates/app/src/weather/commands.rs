//! Wttr.in wrapper — fetches current weather + forecast, caches result in the
//! `setting` table so offline launches have something to show.
//!
//! The user picks a location via Settings → Household (default: empty string,
//! which lets wttr.in infer from IP). Cache key: `today.weather_cache` — a JSON
//! blob with `{ fetched_at, payload }`. TTL is 30 minutes for "fresh", 24 hours
//! for "stale-but-displayable-offline".

use crate::assistant::commands::Db;
use anyhow::Context;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

const LOCATION_KEY: &str = "today.weather_location";
const CACHE_KEY: &str = "today.weather_cache";
const FRESH_TTL_SECONDS: i64 = 30 * 60;
const OFFLINE_TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weather {
    pub temp_c: i32,
    pub condition: String,
    pub emoji: String,
    pub location: String,
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cached {
    fetched_at: i64,
    payload: Weather,
}

// Mapping of wttr.in weatherDesc → emoji. Keeps the UI lightweight.
fn emoji_for(desc: &str) -> &'static str {
    let d = desc.to_lowercase();
    if d.contains("thunder") {
        "⛈"
    } else if d.contains("snow") || d.contains("sleet") {
        "❄️"
    } else if d.contains("rain") || d.contains("drizzle") || d.contains("shower") {
        "🌧"
    } else if d.contains("fog") || d.contains("mist") {
        "🌫"
    } else if d.contains("overcast") {
        "☁️"
    } else if d.contains("cloudy") || d.contains("cloud") {
        "⛅"
    } else if d.contains("clear") || d.contains("sunny") {
        "☀️"
    } else {
        "🌡"
    }
}

#[tauri::command]
pub async fn weather_current(state: State<'_, Db>) -> Result<Option<Weather>, String> {
    // Step 1: read location + cached value from DB.
    let (location, cached): (String, Option<Cached>) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let loc = manor_core::setting::get_or_default(&conn, LOCATION_KEY, "").unwrap_or_default();
        let cache = manor_core::setting::get_json::<Cached>(&conn, CACHE_KEY).unwrap_or(None);
        (loc, cache)
    };
    let now = Utc::now().timestamp();

    // Step 2: if cache is fresh, return it.
    if let Some(c) = &cached {
        if now - c.fetched_at < FRESH_TTL_SECONDS {
            return Ok(Some(c.payload.clone()));
        }
    }

    // Step 3: fetch from wttr.in.
    let url = if location.trim().is_empty() {
        "https://wttr.in/?format=j1".to_string()
    } else {
        format!("https://wttr.in/{}?format=j1", urlencoded(&location))
    };
    let fetch_result = fetch_and_parse(&url, &location).await;

    match fetch_result {
        Ok(fresh) => {
            // Persist to cache.
            let cached = Cached {
                fetched_at: now,
                payload: fresh.clone(),
            };
            let conn = state.0.lock().map_err(|e| e.to_string())?;
            let _ = manor_core::setting::set_json(&conn, CACHE_KEY, &cached);
            Ok(Some(fresh))
        }
        Err(e) => {
            tracing::warn!("weather fetch failed: {e}");
            // Serve stale cache if <24h old.
            if let Some(c) = cached {
                if now - c.fetched_at < OFFLINE_TTL_SECONDS {
                    return Ok(Some(c.payload));
                }
            }
            Ok(None)
        }
    }
}

fn urlencoded(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "+".to_string(),
            c if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') => c.to_string(),
            c => {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf)
                    .bytes()
                    .map(|b| format!("%{b:02X}"))
                    .collect()
            }
        })
        .collect()
}

async fn fetch_and_parse(url: &str, location: &str) -> anyhow::Result<Weather> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let resp = client
        .get(url)
        .header("User-Agent", "manor/0.1")
        .send()
        .await
        .context("weather http send")?;
    anyhow::ensure!(
        resp.status().is_success(),
        "wttr.in returned {}",
        resp.status()
    );
    let body: serde_json::Value = resp.json().await.context("weather json parse")?;
    let current = body
        .get("current_condition")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow::anyhow!("no current_condition"))?;
    let temp_c: i32 = current
        .get("temp_C")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("missing temp_C"))?;
    let condition = current
        .get("weatherDesc")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|o| o.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let emoji = emoji_for(&condition).to_string();
    let display_location = if location.trim().is_empty() {
        body.get("nearest_area")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|o| o.get("areaName"))
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|o| o.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        location.to_string()
    };
    Ok(Weather {
        temp_c,
        condition,
        emoji,
        location: display_location,
        fetched_at: Utc::now().timestamp(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_for_covers_common_conditions() {
        assert_eq!(emoji_for("Thunderstorm"), "⛈");
        assert_eq!(emoji_for("Heavy rain"), "🌧");
        assert_eq!(emoji_for("Light drizzle"), "🌧");
        assert_eq!(emoji_for("Clear"), "☀️");
        assert_eq!(emoji_for("Sunny"), "☀️");
        assert_eq!(emoji_for("Overcast"), "☁️");
        assert_eq!(emoji_for("Partly cloudy"), "⛅");
        assert_eq!(emoji_for("Snowing"), "❄️");
        assert_eq!(emoji_for("Foggy morning"), "🌫");
        assert_eq!(emoji_for("UnknownAlienWeather"), "🌡");
    }

    #[test]
    fn urlencoded_handles_spaces_and_punct() {
        assert_eq!(urlencoded("London"), "London");
        assert_eq!(urlencoded("New York"), "New+York");
        assert_eq!(urlencoded("São Paulo"), "S%C3%A3o+Paulo");
    }
}
