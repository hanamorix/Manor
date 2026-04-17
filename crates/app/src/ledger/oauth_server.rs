//! One-shot loopback HTTP listener for the GoCardless OAuth callback.
//! Spawned on a dedicated OS thread (tiny_http is blocking).
//! Serves one successful /bank-auth?... hit, replies with self-closing HTML,
//! sends callback params back through a oneshot channel, and shuts down.
//!
//! Timeout: 10 minutes. Non-matching paths return 404 and keep the listener alive.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

const TIMEOUT: Duration = Duration::from_secs(600);

pub const SELF_CLOSING_HTML: &str = r#"<!doctype html>
<html><head><title>Manor</title></head>
<body style="font-family:system-ui;background:#1a1a2e;color:#e4e4e7;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
<div style="text-align:center">
  <h1>Connected.</h1>
  <p>You can close this tab &mdash; Manor has taken over.</p>
</div>
<script>setTimeout(() => window.close(), 800);</script>
</body></html>"#;

pub struct LoopbackCallback {
    pub port: u16,
    pub receiver: oneshot::Receiver<Result<HashMap<String, String>>>,
}

/// Start the listener. Returns `(port, receiver)`. The receiver resolves to:
///   Ok(Ok(params))   — callback received
///   Ok(Err(e))       — listener errored (bind, timeout, other)
///   Err(RecvError)   — channel dropped unexpectedly
pub fn start() -> Result<LoopbackCallback> {
    let server = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| anyhow!("bind 127.0.0.1: {e}"))?;
    let addr = server.server_addr();
    let port = addr
        .to_ip()
        .ok_or_else(|| anyhow!("listener not bound to IP socket"))?
        .port();

    let (tx, rx) = oneshot::channel::<Result<HashMap<String, String>>>();

    std::thread::spawn(move || {
        let start = Instant::now();
        let result = run_until_callback(&server, start);
        let _ = tx.send(result);
    });

    Ok(LoopbackCallback { port, receiver: rx })
}

fn run_until_callback(
    server: &tiny_http::Server,
    start: Instant,
) -> Result<HashMap<String, String>> {
    loop {
        let remaining = TIMEOUT.checked_sub(start.elapsed())
            .ok_or_else(|| anyhow!("oauth loopback timed out"))?;
        let req = match server.recv_timeout(remaining) {
            Ok(Some(r)) => r,
            Ok(None) => return Err(anyhow!("oauth loopback timed out")),
            Err(e) => return Err(anyhow!("listener recv: {e}")),
        };
        let url = req.url().to_string();
        if let Some(query) = path_matches(&url, "/bank-auth") {
            let params = parse_query(query);
            let response = tiny_http::Response::from_string(SELF_CLOSING_HTML)
                .with_status_code(200)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                        .unwrap(),
                );
            let _ = req.respond(response);
            return Ok(params);
        }
        let _ = req.respond(tiny_http::Response::from_string("").with_status_code(404));
    }
}

fn path_matches<'a>(url: &'a str, path: &str) -> Option<&'a str> {
    if let Some(rest) = url.strip_prefix(path) {
        match rest.chars().next() {
            None => Some(""),
            Some('?') => Some(&rest[1..]),
            _ => None,
        }
    } else {
        None
    }
}

fn parse_query(q: &str) -> HashMap<String, String> {
    q.split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((
                urlencoding::decode(k).ok()?.into_owned(),
                urlencoding::decode(v).ok()?.into_owned(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn receives_callback_and_shuts_down() {
        let cb = start().unwrap();
        let port = cb.port;

        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/bank-auth?ref=abc&state=xyz");
        let resp = client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.text().await.unwrap();
        assert!(body.contains("Connected."));

        let params = cb.receiver.await.unwrap().unwrap();
        assert_eq!(params.get("ref"), Some(&"abc".to_string()));
        assert_eq!(params.get("state"), Some(&"xyz".to_string()));
    }

    #[tokio::test]
    async fn wrong_path_returns_404_and_keeps_listening() {
        let cb = start().unwrap();
        let port = cb.port;

        let client = reqwest::Client::new();
        let bad = client.get(format!("http://127.0.0.1:{port}/nope")).send().await.unwrap();
        assert_eq!(bad.status(), reqwest::StatusCode::NOT_FOUND);

        let good = client
            .get(format!("http://127.0.0.1:{port}/bank-auth?ref=ok"))
            .send()
            .await
            .unwrap();
        assert_eq!(good.status(), reqwest::StatusCode::OK);

        let params = cb.receiver.await.unwrap().unwrap();
        assert_eq!(params.get("ref"), Some(&"ok".to_string()));
    }
}
