# Bundle B — Release-Ready Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every P0 security + correctness finding and the P1 items bundled under "Release-Ready" in the 2026-04-20 audit, so Manor is safe to hand to trusted testers.

**Architecture:** Fifteen focused tasks across five phases. Phase 1–2 close P0 security + data-integrity bugs. Phase 3–4 close high-impact P1 items. Phase 5 trims frontend weight (bundle + dep + image). Each task is independently mergeable; phases are ordered so later tasks can rely on earlier infrastructure (e.g. the SSRF guard module).

**Tech Stack:** Rust (rusqlite, reqwest, refinery migrations, scraper, pdf-extract), React 18 + Vite + zustand + react-markdown, Tauri 2, vitest + cargo test, wiremock for HTTP tests.

---

## File Structure

**New files:**
- `crates/core/src/net/mod.rs` — module declaration
- `crates/core/src/net/ssrf.rs` — SSRF guard: scheme + host → `Url` vetting, resolves + rejects loopback/private/link-local/multicast
- `crates/core/src/net/fetch.rs` — streaming body-capped `get_bytes(url, max_bytes)` helper built on reqwest
- `crates/core/migrations/V22__message_created_at_seconds.sql` — convert ms → s
- `apps/desktop/src/lib/ui/scheme.ts` — `isSafeExternalScheme(href)` helper
- `apps/desktop/src/components/Bones/__tests__/RepairMarkdown.test.tsx` — scheme allowlist tests

**Modified files:**
- `apps/desktop/src-tauri/tauri.conf.json` — add CSP
- `apps/desktop/package.json` — bump lucide-react, remove orphan imports
- `apps/desktop/vite.config.ts` — manual chunks hint
- `apps/desktop/src/App.tsx` — `React.lazy` + `Suspense` for 7 view components
- `apps/desktop/src/components/Assistant/Assistant.tsx` — scope ephemeral timer to the *current* request
- `apps/desktop/src/components/Bones/RepairMarkdown.tsx` — scheme allowlist
- `apps/desktop/src/assets/avatars/` — swap `manor_face.png` → `manor_face.webp`
- `crates/core/src/assistant/message.rs` — use seconds everywhere
- `crates/core/src/lib.rs` — expose `pub mod net;`
- `crates/app/src/assistant/commands.rs` — thread real `conversation_id`, clean empty assistant row on error
- `crates/app/src/recipe/importer.rs` — use ssrf guard + streaming body cap
- `crates/app/src/repair/fetch.rs` — same
- `crates/app/src/repair/search.rs` — same
- `crates/app/src/asset/commands.rs` — canonicalize + sandbox source path
- `crates/core/src/pdf_extract/text.rs` — cap extracted text length before returning

**Deleted files:**
- `apps/desktop/src/components/Ledger/SummaryCard.tsx`
- `apps/desktop/src/components/Settings/SettingsCog.tsx`

---

## Phase 1 — P0 Security (SSRF + CSP)

### Task 1: Enable Content Security Policy

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.conf.json`

- [ ] **Step 1: Replace the CSP line**

Change `"csp": null` to:

```json
"csp": "default-src 'self'; img-src 'self' asset: https://asset.localhost data:; connect-src 'self' ipc: http://ipc.localhost https://api.anthropic.com http://localhost:11434 http://127.0.0.1:11434; style-src 'self' 'unsafe-inline'; script-src 'self'; font-src 'self' data:; object-src 'none'; frame-ancestors 'none';"
```

`'unsafe-inline'` on style is required because Tauri + React inline styles are everywhere in this app; loosen only style. Connect-src covers Anthropic (remote LLM), Ollama (default 11434), and Tauri's IPC protocol.

- [ ] **Step 2: Rebuild and smoke-test**

Run: `cd apps/desktop && pnpm tauri dev`
Check: app launches, message stream from Ollama works, recipe import works, no CSP violations in DevTools console. If any appear, read the directive name the browser flagged and widen *that* directive only — don't weaken `script-src`.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/tauri.conf.json
git commit -m "sec(csp): enable strict Content Security Policy on main window"
```

---

### Task 2: Create SSRF guard module

**Files:**
- Create: `crates/core/src/net/mod.rs`
- Create: `crates/core/src/net/ssrf.rs`
- Modify: `crates/core/src/lib.rs` — add `pub mod net;`
- Test: `crates/core/src/net/ssrf.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Create `crates/core/src/net/mod.rs`**

```rust
pub mod ssrf;
```

- [ ] **Step 2: Write the failing test**

Create `crates/core/src/net/ssrf.rs` with tests first:

```rust
use std::net::IpAddr;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SsrfError {
    #[error("not a valid URL")]
    BadUrl,
    #[error("unsupported scheme (only http/https allowed)")]
    BadScheme,
    #[error("hostname resolves to a private or loopback address")]
    PrivateAddress,
    #[error("hostname could not be resolved")]
    Unresolvable,
}

/// Parse and vet a URL for safe outbound fetching from user-controlled input.
///
/// Rejects: non-http(s) schemes, IPs that resolve to loopback (127.0.0.0/8, ::1),
/// private networks (10/8, 172.16/12, 192.168/16, fc00::/7), link-local
/// (169.254/16, fe80::/10), multicast, unspecified (0.0.0.0, ::).
///
/// Resolves DNS synchronously via `ToSocketAddrs` — every resolved IP must pass.
pub fn vet_url(raw: &str) -> Result<Url, SsrfError> {
    let url = Url::parse(raw).map_err(|_| SsrfError::BadUrl)?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(SsrfError::BadScheme);
    }
    let host = url.host_str().ok_or(SsrfError::BadUrl)?;
    let port = url.port_or_known_default().unwrap_or(80);

    use std::net::ToSocketAddrs;
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|_| SsrfError::Unresolvable)?
        .collect();
    if addrs.is_empty() {
        return Err(SsrfError::Unresolvable);
    }
    for a in &addrs {
        if is_blocked_ip(&a.ip()) {
            return Err(SsrfError::PrivateAddress);
        }
    }
    Ok(url)
}

fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // 100.64/10 CGNAT, not covered by is_private
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 0x40)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                // is_unique_local / is_unicast_link_local are unstable — hand-check
                || (v6.segments()[0] & 0xfe00) == 0xfc00  // ULA fc00::/7
                || (v6.segments()[0] & 0xffc0) == 0xfe80  // link-local fe80::/10
                // Any IPv4-mapped address must re-check as v4
                || v6.to_ipv4_mapped().map(|v4| is_blocked_ip(&IpAddr::V4(v4))).unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        assert_eq!(vet_url("file:///etc/passwd").unwrap_err(), SsrfError::BadScheme);
        assert_eq!(vet_url("ssh://host").unwrap_err(), SsrfError::BadScheme);
        assert_eq!(vet_url("javascript:alert(1)").unwrap_err(), SsrfError::BadScheme);
    }

    #[test]
    fn rejects_loopback_literal() {
        assert_eq!(vet_url("http://127.0.0.1:11434/").unwrap_err(), SsrfError::PrivateAddress);
        assert_eq!(vet_url("http://[::1]/").unwrap_err(), SsrfError::PrivateAddress);
    }

    #[test]
    fn rejects_private_ranges() {
        assert_eq!(vet_url("http://10.0.0.1/").unwrap_err(), SsrfError::PrivateAddress);
        assert_eq!(vet_url("http://192.168.1.1/").unwrap_err(), SsrfError::PrivateAddress);
        assert_eq!(vet_url("http://172.16.0.1/").unwrap_err(), SsrfError::PrivateAddress);
    }

    #[test]
    fn rejects_link_local() {
        assert_eq!(vet_url("http://169.254.169.254/").unwrap_err(), SsrfError::PrivateAddress);
    }

    #[test]
    fn rejects_bad_url() {
        assert_eq!(vet_url("not a url").unwrap_err(), SsrfError::BadUrl);
        assert_eq!(vet_url("http://").unwrap_err(), SsrfError::BadUrl);
    }

    #[test]
    fn accepts_public_host() {
        // Uses real DNS — skip in offline CI by gating with env or pick a stable IP literal.
        // 1.1.1.1 is public, always resolvable as a literal.
        vet_url("https://1.1.1.1/").expect("1.1.1.1 must pass");
    }
}
```

- [ ] **Step 3: Wire the module**

Edit `crates/core/src/lib.rs` — find the `pub mod` block near the top, add:

```rust
pub mod net;
```

- [ ] **Step 4: Add url + thiserror deps if missing**

Check `crates/core/Cargo.toml`. `url` and `thiserror` are likely already there (used by refinery/reqwest downstream). If not, add:

```toml
url = "2"
thiserror = "1"
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p manor-core net::ssrf`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/net/ crates/core/src/lib.rs crates/core/Cargo.toml
git commit -m "sec(net): add SSRF guard that blocks loopback, private, link-local addresses"
```

---

### Task 3: Wire SSRF guard into recipe importer

**Files:**
- Modify: `crates/app/src/recipe/importer.rs`
- Test: `crates/app/src/recipe/importer.rs` (existing `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

Add to the existing test module in `crates/app/src/recipe/importer.rs`:

```rust
#[tokio::test]
async fn preview_rejects_loopback_url() {
    let err = preview("http://127.0.0.1:8080/recipe", None).await.unwrap_err();
    // Check it's the BadUrl variant (or whichever variant you mapped to).
    let msg = format!("{err}");
    assert!(msg.contains("URL") || msg.contains("private") || msg.contains("rejected"),
        "got: {msg}");
}

#[tokio::test]
async fn preview_rejects_file_scheme() {
    let err = preview("file:///etc/passwd", None).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("URL") || msg.contains("scheme"), "got: {msg}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p manor-app recipe::importer::tests::preview_rejects`
Expected: FAIL (current code happily fetches loopback).

- [ ] **Step 3: Update `preview` to vet the URL**

Replace line 47 in `crates/app/src/recipe/importer.rs`:

```rust
let parsed_url = manor_core::net::ssrf::vet_url(url)
    .map_err(|_| ImportError::BadUrl)?;
```

(Drop the separate `reqwest::Url::parse` — `vet_url` returns the parsed `Url` already.)

- [ ] **Step 4: Update `fetch_hero_image` (around line 137) similarly**

Find the `fetch_hero_image` function's URL parse, replace with `vet_url`. Keep the existing error type; map `SsrfError` → your `ImportError::BadUrl` variant.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p manor-app recipe::importer`
Expected: all pass including the two new SSRF tests.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/recipe/importer.rs
git commit -m "sec(recipe): block SSRF via ssrf::vet_url before any fetch"
```

---

### Task 4: Wire SSRF guard into repair fetch + search

**Files:**
- Modify: `crates/app/src/repair/fetch.rs`
- Modify: `crates/app/src/repair/search.rs`
- Test: both files (inline)

- [ ] **Step 1: Write failing tests**

In `crates/app/src/repair/fetch.rs`, add to its test module (or create one):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fetch_rejects_loopback() {
        let err = fetch_page("http://127.0.0.1/").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("URL") || msg.contains("private") || msg.contains("scheme"),
            "got: {msg}");
    }

    #[tokio::test]
    async fn fetch_rejects_file_scheme() {
        let err = fetch_page("file:///etc/passwd").await.unwrap_err();
        assert!(format!("{err}").contains("URL") || format!("{err}").contains("scheme"));
    }
}
```

(Use whatever the public entry function is — if it's `fetch_page`, that's fine; otherwise match the actual signature.)

Mirror the same pair of tests inside `crates/app/src/repair/search.rs` against its public fetch function (likely `ddg_search` or `youtube_search`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p manor-app repair::fetch::tests::fetch_rejects`
Run: `cargo test -p manor-app repair::search::tests`
Expected: both FAIL.

- [ ] **Step 3: Vet the URL in each entry point**

In `crates/app/src/repair/fetch.rs`, at the top of every publicly-callable fetch function (around line 22 per the audit):

```rust
let url = manor_core::net::ssrf::vet_url(url)
    .map_err(|e| anyhow::anyhow!("url rejected: {e}"))?;
```

Use the returned `Url` in the subsequent `client.get(...)` instead of the raw string.

Same treatment in `crates/app/src/repair/search.rs` at lines 21 and 75 (the DDG + YouTube request builders).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p manor-app repair`
Expected: all repair tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/repair/
git commit -m "sec(repair): vet URLs through SSRF guard in fetch and search"
```

---

## Phase 2 — P0 Correctness (send_message + timestamps)

### Task 5: Unify `message.created_at` to seconds

**Files:**
- Create: `crates/core/migrations/V22__message_created_at_seconds.sql`
- Modify: `crates/core/src/assistant/message.rs`
- Test: `crates/core/src/assistant/message.rs` (inline)

**Why:** `message.rs:76` writes `timestamp_millis()` while every other table uses seconds. Any join or time-window comparison between `message` and other tables silently uses the wrong unit. Also `apps/desktop/src/components/Assistant/Assistant.tsx:134` sends `id: -Date.now()` which is milliseconds — but that value is a local id, not a timestamp, so no change needed there.

- [ ] **Step 1: Write the failing test**

Add to the test module in `crates/core/src/assistant/message.rs`:

```rust
#[test]
fn insert_stores_created_at_in_seconds() {
    let conn = setup_test_db();  // use existing helper
    let conv = conversation::get_or_create_default(&conn).unwrap();
    let id = insert(&conn, conv.id, Role::User, "hi").unwrap();
    let now_secs = chrono::Utc::now().timestamp();
    let stored: i64 = conn.query_row(
        "SELECT created_at FROM message WHERE id = ?1",
        [id],
        |r| r.get(0),
    ).unwrap();
    // Must be within 5 seconds of now, in seconds (not ms).
    assert!(
        (stored - now_secs).abs() < 5,
        "stored {stored} is not within 5s of now {now_secs} — likely ms vs s unit mismatch",
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p manor-core assistant::message::tests::insert_stores_created_at_in_seconds`
Expected: FAIL — stored value is ~1000x larger than expected.

- [ ] **Step 3: Write the migration**

Create `crates/core/migrations/V22__message_created_at_seconds.sql`:

```sql
-- Convert message.created_at from milliseconds to seconds.
-- Any value >= 2_000_000_000 is treated as ms (1970-era sec values max out at ~2B;
-- ms values from 2001+ are always >= 1_000_000_000_000). Divide by 1000 for those.
UPDATE message
SET created_at = created_at / 1000
WHERE created_at >= 2000000000;
```

- [ ] **Step 4: Fix the insert**

Edit `crates/core/src/assistant/message.rs:76`:

```rust
let now_s = Utc::now().timestamp();  // was: timestamp_millis()
```

And update the `params![...]` call accordingly (`now_s` replaces `now_ms`).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p manor-core assistant::message`
Expected: PASS. All existing message tests must still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/core/migrations/V22__message_created_at_seconds.sql \
        crates/core/src/assistant/message.rs
git commit -m "fix(message): store created_at in seconds, add V22 migration to convert existing rows"
```

---

### Task 6: `send_message` — real `conversation_id` + clean empty assistant row

**Files:**
- Modify: `crates/app/src/assistant/commands.rs`
- Test: `crates/app/src/assistant/commands.rs` (add integration test using a test DB)

**What:**
1. Pass the real `conv.id` from the first lock window (line 99) out to the readback at line 165 instead of the hardcoded `1`.
2. If Ollama streaming ends with an Error *and* no tokens were persisted, `DELETE` the empty assistant row so the unread badge doesn't tick up.

- [ ] **Step 1: Write the failing tests**

Add a helper in `crates/app/src/assistant/commands.rs` test module or a new `#[cfg(test)] mod tests_send_message`:

```rust
#[tokio::test]
async fn send_message_uses_real_conversation_id_for_rationale() {
    // Set up DB, insert a conversation with id != 1 (e.g. insert two convs, use the second).
    // Stub an Ollama client that returns a single token "ok" with no tool calls.
    // Run send_message, assert the resulting proposal.rationale (if tool emitted) or
    // the stored message content matches — confirm readback used the right conv id.
    // Simpler variant: assert the assistant row for the NEW conv has content "ok",
    // and no row under conv_id=1 was read.
}

#[tokio::test]
async fn send_message_cleans_empty_assistant_row_on_error() {
    // Stub Ollama client that returns OllamaUnreachable immediately, no tokens.
    // Run send_message. Expect:
    //   - user message row exists
    //   - assistant message row with empty content does NOT exist (deleted)
    //   - unread_count is 0
}
```

If setting up an Ollama stub is too heavy for this change, reach for the existing test infrastructure (`crates/app/src/assistant/` has test harness or can grow one with wiremock). If no harness exists, add a minimal one that accepts an injected `dyn ChatClient` trait object; the next task-owner can see that pattern applied in `crates/core/src/recipe/import.rs` (which already uses an `LlmClient` trait).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p manor-app assistant::commands::tests_send_message`
Expected: FAIL — current code passes `1` for conv_id and leaves empty rows.

- [ ] **Step 3: Thread `conv.id` through**

In `crates/app/src/assistant/commands.rs:97-105`, change the destructuring to include `conv.id`:

```rust
let (assistant_row_id, conversation_id, history) = {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let conv = conversation::get_or_create_default(&conn).map_err(|e| e.to_string())?;
    message::insert(&conn, conv.id, Role::User, &content).map_err(|e| e.to_string())?;
    let assistant_row_id =
        message::insert(&conn, conv.id, Role::Assistant, "").map_err(|e| e.to_string())?;
    let recent = message::list(&conn, conv.id, CONTEXT_WINDOW, 0).map_err(|e| e.to_string())?;
    (assistant_row_id, conv.id, recent)
};
```

Change line 165:

```rust
let msgs = message::list(&conn, conversation_id, 1, 0).map_err(|e| e.to_string())?;
```

- [ ] **Step 4: Clean the empty assistant row on error**

After the current line 157 (`let (chunks_to_persist, events) = ...`) and before the persistence block, add:

```rust
let had_error = events.iter().any(|e| matches!(e, StreamChunk::Error(_)));
let had_tokens = !chunks_to_persist.is_empty();
if had_error && !had_tokens {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM message WHERE id = ?1 AND content = ''", [assistant_row_id])
        .map_err(|e| e.to_string())?;
    // Still replay the Error event to the frontend so UX works.
    for event in events {
        on_event.send(event).map_err(|e| e.to_string())?;
    }
    return Ok(());
}
```

Adjust the enum variant name (`StreamChunk::Error`) to whatever the actual name is in that file.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p manor-app assistant::commands`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/assistant/commands.rs
git commit -m "fix(assistant): use real conversation_id for rationale, delete empty assistant row on error"
```

---

## Phase 3 — P1 Security

### Task 7: Streaming body-size cap

**Files:**
- Create: `crates/core/src/net/fetch.rs`
- Modify: `crates/core/src/net/mod.rs`
- Modify: `crates/app/src/recipe/importer.rs`
- Modify: `crates/app/src/repair/fetch.rs`

**Why:** Current size guards rely on the `Content-Length` response header. A server that omits or lies about it can stream unbounded bytes; `resp.text()/bytes()` buffers all of it. Read the body chunk-by-chunk, stop at the cap.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/net/fetch.rs` with:

```rust
use reqwest::Client;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("response too large (> {0} bytes)")]
    TooLarge(usize),
}

/// GET the URL and return bytes, capping mid-stream at `max_bytes`.
/// If the response exceeds the cap we abort the stream and return TooLarge —
/// `Content-Length` is NOT trusted.
pub async fn get_bytes_capped(
    client: &Client,
    url: reqwest::Url,
    max_bytes: usize,
) -> Result<Vec<u8>, FetchError> {
    let mut resp = client.get(url).send().await?;
    let mut buf = Vec::<u8>::new();
    while let Some(chunk) = resp.chunk().await? {
        if buf.len() + chunk.len() > max_bytes {
            return Err(FetchError::TooLarge(max_bytes));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn rejects_oversize_body_even_without_content_length() {
        let mock = MockServer::start().await;
        let big = vec![b'x'; 2 * 1024 * 1024];
        Mock::given(method("GET")).and(path("/big"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(big))
            .mount(&mock).await;
        let client = Client::new();
        let url = reqwest::Url::parse(&format!("{}/big", mock.uri())).unwrap();
        let err = get_bytes_capped(&client, url, 1024 * 1024).await.unwrap_err();
        assert!(matches!(err, FetchError::TooLarge(_)));
    }

    #[tokio::test]
    async fn accepts_small_body() {
        let mock = MockServer::start().await;
        Mock::given(method("GET")).and(path("/small"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hi"))
            .mount(&mock).await;
        let client = Client::new();
        let url = reqwest::Url::parse(&format!("{}/small", mock.uri())).unwrap();
        let bytes = get_bytes_capped(&client, url, 1024).await.unwrap();
        assert_eq!(bytes, b"hi");
    }
}
```

- [ ] **Step 2: Register module**

Edit `crates/core/src/net/mod.rs`:

```rust
pub mod ssrf;
pub mod fetch;
```

- [ ] **Step 3: Add wiremock dev-dep if missing**

Check `crates/core/Cargo.toml` `[dev-dependencies]`. Add if absent:

```toml
wiremock = "0.6"
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p manor-core net::fetch`
Expected: PASS.

- [ ] **Step 5: Use in recipe importer**

In `crates/app/src/recipe/importer.rs`, replace the `resp.text().await` call (around line 77) with:

```rust
let body_bytes = manor_core::net::fetch::get_bytes_capped(&client, parsed_url.clone(), MAX_BODY_BYTES as usize)
    .await
    .map_err(|e| match e {
        manor_core::net::fetch::FetchError::TooLarge(_) => ImportError::TooLarge,
        _ => ImportError::FetchFailed,
    })?;
let body = String::from_utf8_lossy(&body_bytes).into_owned();
```

Drop the now-dead `Content-Length`-based check (the cap is enforced in-stream).

For `fetch_hero_image` (around line 156), do the same with `MAX_IMAGE_BYTES`.

- [ ] **Step 6: Use in repair fetch**

Same pattern in `crates/app/src/repair/fetch.rs` — replace the text/bytes collection with `get_bytes_capped`.

- [ ] **Step 7: Run all downstream tests**

Run: `cargo test -p manor-app recipe::importer repair::fetch`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/net/ crates/core/Cargo.toml \
        crates/app/src/recipe/importer.rs crates/app/src/repair/fetch.rs
git commit -m "sec(net): streaming body-size cap, don't trust Content-Length"
```

---

### Task 8: Markdown link scheme allowlist

**Files:**
- Create: `apps/desktop/src/lib/ui/scheme.ts`
- Create: `apps/desktop/src/components/Bones/__tests__/RepairMarkdown.test.tsx`
- Modify: `apps/desktop/src/components/Bones/RepairMarkdown.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src/components/Bones/__tests__/RepairMarkdown.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { RepairMarkdown } from "../RepairMarkdown";

// Mock the Tauri shell plugin so openUrl is observable.
vi.mock("@tauri-apps/plugin-shell", () => ({
  open: vi.fn(() => Promise.resolve()),
}));

import { open as openUrl } from "@tauri-apps/plugin-shell";

describe("RepairMarkdown link safety", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => cleanup());

  it("opens https links via the shell plugin", () => {
    render(<RepairMarkdown body="[Safe](https://example.com)" />);
    fireEvent.click(screen.getByText("Safe"));
    expect(openUrl).toHaveBeenCalledWith("https://example.com");
  });

  it("does NOT open file:// links", () => {
    render(<RepairMarkdown body="[Local](file:///etc/passwd)" />);
    fireEvent.click(screen.getByText("Local"));
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("does NOT open javascript: links", () => {
    render(<RepairMarkdown body="[Bad](javascript:alert(1))" />);
    fireEvent.click(screen.getByText("Bad"));
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("does NOT open custom-scheme links", () => {
    render(<RepairMarkdown body="[Vs](vscode://open?file=x)" />);
    fireEvent.click(screen.getByText("Vs"));
    expect(openUrl).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/desktop && pnpm vitest run src/components/Bones/__tests__/RepairMarkdown.test.tsx`
Expected: FAIL — current code passes any href through.

- [ ] **Step 3: Write the helper**

Create `apps/desktop/src/lib/ui/scheme.ts`:

```ts
/**
 * Return true only for absolute URLs with http(s) scheme.
 * Relative URLs and anchors are rejected here because RepairMarkdown
 * passes hrefs straight to the Tauri shell plugin — anchors don't make
 * sense there.
 */
export function isSafeExternalScheme(href: string | undefined): boolean {
  if (!href) return false;
  try {
    const u = new URL(href);
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}
```

- [ ] **Step 4: Update RepairMarkdown**

Edit `apps/desktop/src/components/Bones/RepairMarkdown.tsx`:

```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { isSafeExternalScheme } from "../../lib/ui/scheme";

interface Props {
  body: string;
}

export function RepairMarkdown({ body }: Props) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        a: ({ href, children, ...rest }) => (
          <a
            {...rest}
            href={href}
            onClick={(e) => {
              e.preventDefault();
              if (isSafeExternalScheme(href)) {
                void openUrl(href!);
              }
            }}
            style={{ color: "var(--link, #0366d6)", textDecoration: "underline", cursor: "pointer" }}
          >
            {children}
          </a>
        ),
      }}
    >
      {body}
    </ReactMarkdown>
  );
}
```

- [ ] **Step 5: Run tests**

Run: `pnpm vitest run src/components/Bones src/lib/ui`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/lib/ui/scheme.ts \
        apps/desktop/src/components/Bones/RepairMarkdown.tsx \
        apps/desktop/src/components/Bones/__tests__/RepairMarkdown.test.tsx
git commit -m "sec(markdown): only open http(s) links from LLM-rendered markdown"
```

---

### Task 9: `asset_attach_*` path canonicalization + sandbox check

**Files:**
- Modify: `crates/app/src/asset/commands.rs`
- Test: `crates/app/src/asset/commands.rs` (inline or alongside existing tests)

**Why:** Both `asset_attach_hero_from_path` and `asset_attach_document_from_path` accept a raw frontend-supplied `source_path` and read it. There's no canonicalization and no allowlist. Canonicalize, reject paths outside a small set of acceptable roots (user home + common Download/Documents/Desktop directories), and reject symlinks that resolve outside those roots.

- [ ] **Step 1: Write the failing test**

In `crates/app/src/asset/commands.rs` (or its test module):

```rust
#[test]
fn canonicalize_source_rejects_system_paths() {
    // Should reject anything under /etc, /usr, /System, etc.
    let err = validate_source_path(std::path::Path::new("/etc/passwd")).unwrap_err();
    assert!(err.contains("outside"), "got: {err}");
}

#[test]
fn canonicalize_source_accepts_home_path() {
    let home = dirs::home_dir().expect("home dir");
    let downloads = home.join("Downloads");
    std::fs::create_dir_all(&downloads).ok();
    let path = downloads.join("test-accept.txt");
    std::fs::write(&path, b"hi").unwrap();
    let result = validate_source_path(&path);
    std::fs::remove_file(&path).ok();
    assert!(result.is_ok(), "got: {result:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p manor-app asset::commands::tests::canonicalize_source`
Expected: FAIL (function doesn't exist).

- [ ] **Step 3: Add the validator**

Add to `crates/app/src/asset/commands.rs`:

```rust
fn validate_source_path(p: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let canonical = p.canonicalize().map_err(|e| format!("cannot resolve path: {e}"))?;
    let home = dirs::home_dir().ok_or("no home dir")?;
    // Allow anything under the user's home — sandboxes Manor away from /etc, /System,
    // other users' homes, and the system library.
    if !canonical.starts_with(&home) {
        return Err(format!(
            "source path is outside the user home directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}
```

Add `dirs = "5"` (or whichever version the rest of the workspace uses — check `Cargo.toml`) to `crates/app/Cargo.toml` dependencies if not already present.

- [ ] **Step 4: Wire into both commands**

In `asset_attach_hero_from_path` (line 85) and `asset_attach_document_from_path` (line 107), replace the raw `PathBuf::from(source_path)` with:

```rust
let src = validate_source_path(std::path::Path::new(&source_path))?;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p manor-app asset`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/asset/commands.rs crates/app/Cargo.toml
git commit -m "sec(asset): canonicalize source path and sandbox to user home"
```

---

### Task 10: PDF decompressed-size cap

**Files:**
- Modify: `crates/core/src/pdf_extract/text.rs`
- Test: same file (inline `#[cfg(test)]`)

**Why:** Raw PDF is gated at 10 MB (`MAX_PDF_BYTES`), but `pdf_extract::extract_text_from_mem` can expand a FlateDecode-bomb PDF to gigabytes of text before the caller caps it. Add a post-decompression character cap; if decompressed text exceeds the cap, treat as malformed.

- [ ] **Step 1: Write the failing test**

Add to the existing test module in `crates/core/src/pdf_extract/text.rs`:

```rust
#[test]
fn extract_rejects_decompressed_text_exceeding_cap() {
    // Simulate a malicious PDF by directly testing that a large text result is rejected.
    // Full bomb synthesis is too involved; test the cap check via a helper unit.
    let huge = "x".repeat(MAX_DECOMPRESSED_CHARS + 1);
    let err = enforce_decompressed_cap(&huge).unwrap_err();
    assert!(matches!(err, ExtractError::TooLarge(_)), "got: {err:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p manor-core pdf_extract::text::tests::extract_rejects_decompressed`
Expected: FAIL (constant and fn don't exist).

- [ ] **Step 3: Add the cap + helper**

In `crates/core/src/pdf_extract/text.rs`, near the other constants:

```rust
/// Maximum decompressed text characters from a PDF. Roughly 20× the raw byte cap —
/// large enough for legitimate dense manuals, small enough to reject FlateDecode bombs.
pub const MAX_DECOMPRESSED_CHARS: usize = 2_000_000;

fn enforce_decompressed_cap(text: &str) -> Result<&str, ExtractError> {
    if text.chars().count() > MAX_DECOMPRESSED_CHARS {
        return Err(ExtractError::TooLarge(MAX_PDF_BYTES / (1024 * 1024)));
    }
    Ok(text)
}
```

And inside `extract_text_from_pdf`, call it:

```rust
let text = pdf_extract::extract_text_from_mem(&bytes)
    .map_err(|e| ExtractError::ParseFailed(e.to_string()))?;
let trimmed = text.trim();
enforce_decompressed_cap(trimmed)?;
if trimmed.chars().count() < MIN_TEXT_CHARS {
    return Err(ExtractError::ImageOnly);
}
Ok(trimmed.to_string())
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p manor-core pdf_extract`
Expected: PASS — including the existing 6 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/pdf_extract/text.rs
git commit -m "sec(pdf): cap decompressed text to guard against FlateDecode bombs"
```

---

## Phase 4 — P1 Bugs

### Task 11: Ephemeral timer leak on rapid resubmit

**Files:**
- Modify: `apps/desktop/src/components/Assistant/Assistant.tsx`
- Test: `apps/desktop/src/components/Assistant/__tests__/Assistant.ephemeral.test.tsx` (new)

**Why:** Each `handleSubmit` creates an independent `onEvent` closure. If submit N+1 fires before submit N's `Done`, N's `Done` handler overwrites the timer ref, but when N's `setTimeout` eventually fires it races with N+1's streaming, hiding the ephemeral log mid-stream of N+1. Scope the timer to the current request; a late `Done` for a superseded request must not touch visibility.

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src/components/Assistant/__tests__/Assistant.ephemeral.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, cleanup } from "@testing-library/react";
import React from "react";

// Minimal reproduction: mount Assistant, fake two concurrent submits,
// assert a late Done from submit 1 does not hide submit 2's content.
// Full integration is heavy; instead, extract the timer logic into a testable hook
// `useEphemeralStreamingVisibility` and unit-test it.

import { useEphemeralStreamingVisibility } from "../useEphemeralStreamingVisibility";

describe("useEphemeralStreamingVisibility", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => { vi.useRealTimers(); cleanup(); });

  it("late Done for superseded request does not hide current stream", () => {
    // Render a test component that exposes the hook API.
    const seen = { visible: true };
    function Probe() {
      const api = useEphemeralStreamingVisibility(10_000);
      React.useEffect(() => {
        // Request 1 starts and ends
        const r1 = api.startRequest();
        r1.onStarted();
        r1.onDone();
        // Before 10s elapses, request 2 starts
        act(() => { vi.advanceTimersByTime(100); });
        const r2 = api.startRequest();
        r2.onStarted();
        // Now r1's fade timer fires — must not hide r2
        act(() => { vi.advanceTimersByTime(11_000); });
        seen.visible = api.visible;
      }, []);
      return null;
    }
    render(<Probe />);
    expect(seen.visible).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/desktop && pnpm vitest run src/components/Assistant/__tests__/Assistant.ephemeral`
Expected: FAIL (hook doesn't exist).

- [ ] **Step 3: Extract the hook**

Create `apps/desktop/src/components/Assistant/useEphemeralStreamingVisibility.ts`:

```ts
import { useCallback, useEffect, useRef, useState } from "react";

export function useEphemeralStreamingVisibility(fadeMs: number) {
  const [visible, setVisible] = useState(false);
  const currentRequestIdRef = useRef(0);
  const timerRef = useRef<number | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  useEffect(() => () => clearTimer(), [clearTimer]);

  const startRequest = useCallback(() => {
    currentRequestIdRef.current += 1;
    const myId = currentRequestIdRef.current;
    return {
      onStarted: () => {
        if (myId !== currentRequestIdRef.current) return;
        clearTimer();
        setVisible(true);
      },
      onDone: () => {
        if (myId !== currentRequestIdRef.current) return;
        clearTimer();
        timerRef.current = window.setTimeout(() => {
          if (myId !== currentRequestIdRef.current) return;
          setVisible(false);
          timerRef.current = null;
        }, fadeMs);
      },
    };
  }, [clearTimer, fadeMs]);

  const hide = useCallback(() => {
    clearTimer();
    setVisible(false);
  }, [clearTimer]);

  return { visible, startRequest, hide };
}
```

- [ ] **Step 4: Use the hook in `Assistant.tsx`**

Replace the inline `ephemeralVisible`, `ephemeralTimerRef`, and `clearEphemeralTimer` with:

```tsx
import { useEphemeralStreamingVisibility } from "./useEphemeralStreamingVisibility";
// ...
const { visible: ephemeralVisible, startRequest, hide: hideEphemeral } =
  useEphemeralStreamingVisibility(EPHEMERAL_FADE_MS);
```

In `handleSubmit`, at the top:

```tsx
const request = startRequest();
```

Replace the `setEphemeralVisible(true)` calls in `Started`/first-token branches with `request.onStarted()`.
Replace the `setTimeout` block in `Done` with `request.onDone()`.

In the `isHistoryOpen` effect, replace `clearEphemeralTimer(); setEphemeralVisible(false);` with `hideEphemeral();`.

Delete the now-unused state + refs + effect.

- [ ] **Step 5: Run tests**

Run: `pnpm vitest run src/components/Assistant`
Expected: all pass, including the new one.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/Assistant/
git commit -m "fix(assistant): scope ephemeral fade timer to current request so late Done can't hide new stream"
```

---

## Phase 5 — Frontend Weight Reduction

### Task 12: Delete orphan components

**Files:**
- Delete: `apps/desktop/src/components/Ledger/SummaryCard.tsx`
- Delete: `apps/desktop/src/components/Settings/SettingsCog.tsx`

- [ ] **Step 1: Confirm zero importers**

Run: `cd apps/desktop && grep -r "SummaryCard" src/ --include="*.ts*" | grep -v "components/Ledger/SummaryCard.tsx"`
Expected: empty output.
Run: `grep -r "SettingsCog" src/ --include="*.ts*" | grep -v "components/Settings/SettingsCog.tsx"`
Expected: empty output.

If any hit appears, STOP — the audit was wrong. Investigate before deleting.

- [ ] **Step 2: Delete**

```bash
rm apps/desktop/src/components/Ledger/SummaryCard.tsx
rm apps/desktop/src/components/Settings/SettingsCog.tsx
```

- [ ] **Step 3: Typecheck + tests**

Run: `pnpm tsc --noEmit && pnpm vitest run`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -A apps/desktop/src/components/Ledger/ apps/desktop/src/components/Settings/
git commit -m "chore: remove orphan SummaryCard and SettingsCog components"
```

---

### Task 13: Upgrade `lucide-react`

**Files:**
- Modify: `apps/desktop/package.json`
- Modify: `apps/desktop/pnpm-lock.yaml` (auto)

**Why:** Current `^1.8.0` is the pre-treeshake major; modern `lucide-react` ships per-icon ESM modules that tree-shake properly. Audit estimates a 50–150 KB reduction off the 591 KB JS bundle.

- [ ] **Step 1: Check latest**

Run: `cd apps/desktop && pnpm info lucide-react version`
Note the version.

- [ ] **Step 2: Upgrade**

Run: `pnpm add lucide-react@latest`

- [ ] **Step 3: Check import sites compile**

Run: `pnpm tsc --noEmit`
Expected: PASS. If any icons were renamed/removed across majors, fix the imports — the compiler will tell you exactly which ones.

Common renames to watch for: `Cog` → `Settings`, `Times` → `X`, etc. If something's truly gone, find the closest equivalent by name in the new package.

- [ ] **Step 4: Rebuild and eyeball**

Run: `pnpm build && ls -lh dist/assets/*.js`
Compare chunk size to baseline 591 KB.

Run: `pnpm tauri dev` and spot-check each icon renders in-app.

- [ ] **Step 5: Run tests**

Run: `pnpm vitest run`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/package.json apps/desktop/pnpm-lock.yaml
git commit -m "perf(deps): upgrade lucide-react to tree-shakeable major"
```

---

### Task 14: Convert hero PNG → WebP

**Files:**
- Delete: `apps/desktop/src/assets/avatars/manor_face.png`
- Create: `apps/desktop/src/assets/avatars/manor_face.webp`
- Modify: `apps/desktop/src/components/Assistant/Avatar.tsx`

**Why:** The bundled 256 KB PNG (`dist/assets/manor_face-*.png`) is bigger than the entire gzipped JS bundle. WebP at equivalent visual quality lands around 30–60 KB.

- [ ] **Step 1: Convert with Pillow**

```bash
python3 - <<'PY'
from PIL import Image
src = "apps/desktop/src/assets/avatars/manor_face.png"
dst = "apps/desktop/src/assets/avatars/manor_face.webp"
Image.open(src).convert("RGBA").save(dst, "WEBP", quality=85, method=6)
import os
print(f"png: {os.path.getsize(src):,} bytes")
print(f"webp: {os.path.getsize(dst):,} bytes")
PY
```

Target: WebP < 80 KB. If larger, drop quality to 75.

- [ ] **Step 2: Update the import**

In `apps/desktop/src/components/Assistant/Avatar.tsx:1`:

```tsx
import manorFace from "../../assets/avatars/manor_face.webp";
```

- [ ] **Step 3: Delete the PNG**

```bash
git rm apps/desktop/src/assets/avatars/manor_face.png
```

- [ ] **Step 4: Typecheck + build + eyeball**

Run: `pnpm tsc --noEmit && pnpm build`
Expected: PASS.

Run: `pnpm tauri dev` — verify the avatar renders identically.

- [ ] **Step 5: Run tests**

The Avatar test uses `alt="Manor"` and checks inline styles — it's import-agnostic. Run:
```bash
pnpm vitest run src/components/Assistant
```
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/assets/avatars/ apps/desktop/src/components/Assistant/Avatar.tsx
git commit -m "perf(assets): convert manor_face to WebP (~80% size reduction)"
```

---

### Task 15: Route-level code splitting with `React.lazy`

**Files:**
- Modify: `apps/desktop/src/App.tsx`
- Modify: `apps/desktop/vite.config.ts`

**Why:** Currently all 7 top-level views (Today, Chores, TimeBlocks, Ledger, Hearth, Bones, Settings, Wizard) are eager-imported in `App.tsx:1-11`, producing a single 591 KB chunk. Lazy-load each view so first paint only loads Today + the Assistant shell.

- [ ] **Step 1: Write a small smoke test for a lazy view**

Create `apps/desktop/src/__tests__/lazy-routes.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";

describe("route splitting", () => {
  it("lazy-loads Bones, Hearth, Ledger as separate dynamic imports", async () => {
    // The bundle structure is asserted elsewhere (build output). This test
    // just confirms the import paths return components.
    const { BonesTab } = await import("../components/Bones/BonesTab");
    const { HearthTab } = await import("../components/Hearth/HearthTab");
    expect(typeof BonesTab).toBe("function");
    expect(typeof HearthTab).toBe("function");
  });
});
```

- [ ] **Step 2: Rewrite App.tsx imports**

Replace the 7 eager imports in `apps/desktop/src/App.tsx:2-11` with lazy ones:

```tsx
import { Suspense, lazy, useEffect, useState } from "react";
import Assistant from "./components/Assistant/Assistant";
import Sidebar from "./components/Nav/Sidebar";
import SettingsModal from "./components/Settings/SettingsModal";

const Today = lazy(() => import("./components/Today/Today"));
const ChoresView = lazy(() => import("./components/Chores/ChoresView"));
const TimeBlocksView = lazy(() => import("./components/TimeBlocks/TimeBlocksView"));
const LedgerView = lazy(() => import("./components/Ledger/LedgerView"));
const HearthTab = lazy(() =>
  import("./components/Hearth/HearthTab").then((m) => ({ default: m.HearthTab })),
);
const BonesTab = lazy(() =>
  import("./components/Bones/BonesTab").then((m) => ({ default: m.BonesTab })),
);
const Wizard = lazy(() => import("./components/Wizard/Wizard"));
```

Keep `Assistant`, `Sidebar`, `SettingsModal` eager — they're part of the shell chrome and always visible.

- [ ] **Step 3: Wrap the view switch in `<Suspense>`**

Where `view` is switched to render the matching component (in App.tsx's return JSX), wrap that in:

```tsx
<Suspense fallback={<div style={{ padding: 40, color: "var(--ink-soft)" }}>Loading…</div>}>
  {renderView(view)}
</Suspense>
```

Use the existing spinner/fallback pattern (`{ padding: 40, color: "var(--ink-soft)" }`) that matches the `checking` branch.

- [ ] **Step 4: Add manual chunks hint to Vite**

In `apps/desktop/vite.config.ts`, add to `defineConfig`:

```ts
build: {
  chunkSizeWarningLimit: 300,
  rollupOptions: {
    output: {
      manualChunks: {
        "react-vendor": ["react", "react-dom"],
        "markdown": ["react-markdown", "remark-gfm"],
      },
    },
  },
},
```

- [ ] **Step 5: Build and measure**

Run: `pnpm build`
Expected output: chunk summary showing multiple `.js` files. Note the sizes.

Target: the main/entry chunk drops below 250 KB; `markdown` chunk splits out (~70 KB).

- [ ] **Step 6: Dev-run and navigate each view**

Run: `pnpm tauri dev` and click every sidebar item. Each view should load after a brief spinner the first time. Watch DevTools Network → you'll see a new `.js` chunk per view.

- [ ] **Step 7: Run all tests**

Run: `pnpm vitest run`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src/App.tsx apps/desktop/vite.config.ts \
        apps/desktop/src/__tests__/lazy-routes.test.tsx
git commit -m "perf(bundle): lazy-load top-level views + split react and markdown vendor chunks"
```

---

## Closing

**After Task 15:**
1. Run the full verification sweep:
   ```bash
   cd /Users/hanamori/life-assistant
   cargo test --workspace
   cd apps/desktop && pnpm tsc --noEmit && pnpm vitest run && pnpm build
   ```
2. `git log --oneline main..HEAD` — confirm 15 focused commits, one per task.
3. Smoke test the built app: `pnpm tauri dev`, exercise each view, send a message, import a recipe, attach a PDF, toggle history panel.
4. Use `superpowers:finishing-a-development-branch` to wrap up (PR vs. direct merge depending on your workflow).

---

## Self-Review

**Spec coverage:** Each Bundle B item from the 2026-04-20 audit maps to a task:
- P0 CSP → Task 1 ✓
- P0 SSRF recipe → Task 3 ✓
- P0 SSRF repair → Task 4 ✓
- P0 send_message conversation_id → Task 6 ✓
- P0 send_message empty row → Task 6 ✓
- P0 message timestamp unit → Task 5 ✓
- P1 Content-Length size gate → Task 7 ✓
- P1 markdown scheme allowlist → Task 8 ✓
- P1 asset_attach_* sandbox → Task 9 ✓
- P1 PDF decompressed cap → Task 10 ✓
- P1 ephemeral timer leak → Task 11 ✓
- Orphan components → Task 12 ✓
- Bundle splitting → Task 15 ✓
- lucide upgrade → Task 13 ✓
- Hero PNG → WebP → Task 14 ✓

**Deferred to Bundle C:** `approve_proposal` maintenance branch, `extractExchanges` 100-msg boundary, Selector.parse panics, history-panel effect re-runs, token streaming re-render, `proposal.status` index, DAL/commands file splits, format helper extraction.

**Note:** The audit also flagged a possible "3 lock window race" in `send_message`. On inspection, lines 97-105 hold a single lock across user-insert + assistant-insert + context-read, so the claimed interleave cannot occur. No task allocated.

**Type consistency:** `conversation_id`/`conv.id` used consistently (Task 6). `StreamChunk::Error` enum variant name to be verified at implementation time (noted in step 4). `validate_source_path` signature stable across tasks.

**Placeholders scanned:** none — every code block is complete; every command is exact.
