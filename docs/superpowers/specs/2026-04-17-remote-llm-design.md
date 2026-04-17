# Landmark 2 — Remote LLM Support — Design Spec

- **Date**: 2026-04-17
- **Status**: Draft — pending Hana approval
- **Authors**: Hana (product), Nell (architect/engineer)
- **Parent**: `2026-04-14-life-assistant-design.md` §5
- **Roadmap**: `2026-04-16-gap-closure-roadmap.md` Landmark 2

## Goal

Let Manor call remote LLMs (Claude first; OpenAI/Gemini/Groq later) when the user configures an API key, **without ever sending unredacted personal data off their Mac**. Every remote call is audited. The user sees a running monthly spend + a hard stop when they hit their cap. No remote call is a silent default — each one goes through a tier gate the user controls.

Out of scope: v1.0 Companion cloud sync, mandatory usage, any remote-first features, the `#[ai_tool]` macro ecosystem, full YAML context recipes (§5.5 of parent spec).

---

## 1. The provider abstraction

### 1.1 Trait shape

```rust
#[async_trait]
pub trait RemoteProvider: Send + Sync {
    fn name(&self) -> &str;                    // e.g., "claude"
    fn default_model(&self) -> &str;           // e.g., "claude-sonnet-4-6"
    fn cost_per_million_input_tokens_pence(&self) -> i64;   // static table
    fn cost_per_million_output_tokens_pence(&self) -> i64;

    async fn chat(
        &self,
        api_key: &str,
        model: &str,
        messages: &[ChatMessage],
        system: Option<&str>,
        max_tokens: i64,
    ) -> Result<ChatResponse>;
}

pub struct ChatResponse {
    pub text: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
}
```

### 1.2 Providers in Landmark 2

- **Claude** (full implementation — `claude-sonnet-4-6` as default; `claude-opus-4-7` and `claude-haiku-4-5-20251001` selectable). Uses Anthropic's `/v1/messages` endpoint. Landmark 2 ships this.
- **OpenAI / Gemini / Groq / OpenRouter** — deferred. Same trait, same tests, but the redaction + logging pipeline is what's risky; one provider proves it. Adding the others later is mechanical.

### 1.3 Cost table

Static in code, updated manually per provider. Claude (as of 2026-04-17):
- Opus 4.7: £2.40 input / £12.00 output per M tokens
- Sonnet 4.6: £0.80 input / £4.00 output per M tokens
- Haiku 4.5: £0.12 input / £0.60 output per M tokens

Stored at compile time, not in DB. If prices change, we ship a version. Mismatch at runtime is logged and the call still completes (we'd rather show slightly stale cost than refuse).

---

## 2. The redaction pipeline

This is the load-bearing privacy primitive. If redaction leaks, the whole model collapses.

### 2.1 What gets redacted

From parent spec §5.6 + additions:

| Pattern | Replaced with |
|---|---|
| Credit card numbers (Luhn-valid 13-19 digits, common spacings) | `[REDACTED-CARD]` |
| UK sort codes (`NN-NN-NN`) + following 8-digit account numbers | `[REDACTED-ACCOUNT]` |
| IBANs | `[REDACTED-IBAN]` |
| Email addresses | `user@[EMAIL-HOSTHASH-xxxx]` (first-4 of sha256 of domain) |
| Phone numbers (UK + E.164) | `[REDACTED-PHONE]` |
| UK postcodes | keep first half only (e.g., `NW1 4AB` → `NW1`) |
| National Insurance numbers (UK format `AB 12 34 56 C`) | `[REDACTED-NI]` |

**Not redacted** (intentional): people's names, event titles, tasks, amounts in pence (the model needs them to reason about spend), dates, locations above street level.

### 2.2 The redactor interface

```rust
pub fn redact(input: &str) -> Redacted {
    // Returns Redacted { text, replacements: Vec<Redaction> }
}

pub struct Redacted {
    pub text: String,
    pub replacements: Vec<Redaction>, // for audit + un-redact if caller reverses
}

pub struct Redaction {
    pub kind: &'static str,   // "card" | "account" | "email" | ...
    pub original_hash: String, // sha256 of original for audit (not reversible)
    pub placeholder: String,  // what was inserted
}
```

### 2.3 Per-skill extensions

Skills can pass additional patterns. v0.1 extensions:
- **Ledger** strips low-threshold merchant names (< £5 transactions with short descriptions — probably just "Amazon" or similar brand names; noise to send, not signal).
- **Home** (v0.5) will strip asset serial numbers. Not in Landmark 2 scope.

### 2.4 Property tests — the whole reason this subsystem exists

Property tests hit the redactor with:
- Random strings containing planted PII
- Assertion: after redact(), the text does NOT contain the original PII substring (using `!text.contains(&pii)`)
- Assertion: the redaction count matches the number of planted PII items

This is non-negotiable. Landmark 2 does not ship without property tests passing.

---

## 3. Keychain key storage

### 3.1 Naming

Keychain service: `manor-remote`. Account: `{provider}-api-key`. So Claude's key is stored as `(service="manor-remote", account="claude-api-key")`.

This keeps Manor's keychain footprint readable — the user can inspect Keychain.app and see exactly what Manor is storing.

### 3.2 UI surface

Settings → AI tab gains a "Remote providers" section that lists providers, shows key-set status (not the key itself), and has "Set key" / "Remove key" actions. Setting a key prompts for the value in a password field, stores it, never displays it again.

### 3.3 Keys never leave the Mac in plaintext beyond the one outgoing HTTPS request

No logs, no env vars written to disk, no UI that renders the key. The key leaves memory only via `reqwest`'s TLS-wrapped request body.

---

## 4. `remote_call_log` schema + audit

### 4.1 Table

```sql
CREATE TABLE remote_call_log (
    id                  INTEGER PRIMARY KEY,
    provider            TEXT    NOT NULL,       -- 'claude' | 'openai' | ...
    model               TEXT    NOT NULL,       -- 'claude-sonnet-4-6'
    skill               TEXT    NOT NULL,       -- 'ledger_review' | 'test' | 'unknown'
    user_visible_reason TEXT    NOT NULL,       -- shown in the call log UI
    prompt_redacted     TEXT    NOT NULL,       -- the redacted prompt we sent
    response_text       TEXT,                   -- nullable; NULL if call errored
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cost_pence          INTEGER,                -- computed at log-time from cost table
    redaction_count     INTEGER NOT NULL DEFAULT 0,
    error               TEXT,                   -- nullable; stores provider error
    started_at          INTEGER NOT NULL,
    completed_at        INTEGER,                -- nullable
    deleted_at          INTEGER                 -- soft-delete for Trash
);

CREATE INDEX idx_remote_call_log_month ON remote_call_log(started_at)
  WHERE deleted_at IS NULL;
```

### 4.2 What's stored

- `prompt_redacted`: the exact bytes we sent. If it contains an `[REDACTED-CARD]` placeholder, that's what's here — matches what went to the provider.
- `response_text`: the provider's response. Kept verbatim; provider might echo something surprising.
- `cost_pence`: computed at log time from the provider's cost-per-M-tokens × tokens, rounded up (never round down pence — user sees larger spend, not smaller).
- `error`: if the call failed (timeout, auth, 4xx, 5xx), the error chain. Soft-deletable with `deleted_at` so the user can scrub their log from Settings → Data & Backup → Trash.

### 4.3 What's NOT stored

- The original unredacted prompt. It never touches persistent storage.
- The API key (obviously).
- Request/response bodies' binary metadata (timing histograms, TLS session info, etc.).

---

## 5. Budget guardrails

### 5.1 The numbers

- Per-provider monthly cap in pence. Stored in `setting` as `budget.{provider}_monthly_pence` (default `1000` = £10).
- Monthly reset: calendar month, Europe/London timezone.
- Warning threshold: 75% of cap.
- Hard stop: 100% of cap. Remote calls refuse until user raises cap or enters a new month.

### 5.2 Who enforces

The `remote_chat` orchestrator:
1. Sums `cost_pence` from `remote_call_log` for the current month + provider.
2. If sum ≥ cap → refuse the call with `BudgetExceeded`. Logs an error-row with `cost_pence = 0` so the refusal is auditable.
3. If sum ≥ 75% cap → flags `warn: true` in the response (UI surfaces an amber warning, doesn't block).
4. After a successful call, the new cost_pence is added to the running total for future calls.

### 5.3 Raising the cap

User sets `budget.{provider}_monthly_pence` via the Settings UI. No approval flow, no audit — it's their money. Warning persists as long as they're over 75% of the new value.

### 5.4 Per-request estimate

Landmark 2 does **not** show a per-request cost estimate before the call. Reason: estimating output tokens without running the model is error-prone, and Manor doesn't initiate remote calls from user-facing buttons yet (only the ledger review, which is a known-short response). When remote calls become more common + more variable, add the estimate.

---

## 6. Tier routing — the minimum viable version

Parent spec §5.1 lays out a full 3-tier system with per-skill overrides and "use better AI" one-taps. Landmark 2 ships a **minimum viable** version:

- A single `setting` key `ai.remote.enabled_for_review` (boolean) that controls whether the ledger AI month review uses remote Claude instead of local Ollama.
- If `true` AND a Claude key is set AND budget not exceeded → route to Claude.
- Otherwise → route to local Ollama (existing path).

Full tier routing + per-skill overrides + "use better AI" one-tap are added when there are more consumers. Building the routing system before a second consumer exists is premature generalization.

---

## 7. Settings → AI tab upgrade

Before Landmark 2: tab has Ollama status, default model selector, embeddings section (live), remote providers placeholder.

After Landmark 2: add a new "Remote providers" section with:
- **Claude row**: status (key set / no key), "Set key" / "Remove key" button, provider label, default model name.
- **Monthly budget**: input field (£), current month spend (£), progress bar (green < 75%, amber 75-100%, red ≥ 100%).
- **Use remote for ledger review** toggle: flips `ai.remote.enabled_for_review`.

Plus a new "Call log" subsection below:
- Last N entries from `remote_call_log` (newest first, 20-row limit).
- Each row: timestamp, provider/model, skill, redaction count, cost, outcome (ok / error). Click row to expand: show `prompt_redacted` + `response_text` + `error` (if any).
- "Clear call log" button — soft-deletes all rows (they end up in Trash per Phase B aggregator, auto-emptied on the standard 30-day cycle).

---

## 8. File map (design-only, no implementation)

| Path | Responsibility |
|---|---|
| `crates/core/migrations/V11__remote_call_log.sql` | Schema + index |
| `crates/core/src/remote_call_log.rs` | DAL: insert_started, mark_completed, mark_errored, list_recent, sum_month_pence, clear_all |
| `crates/core/src/redact.rs` | `redact(input: &str) -> Redacted` + property-tested patterns |
| `crates/app/src/remote/mod.rs` | Module root |
| `crates/app/src/remote/provider.rs` | `RemoteProvider` trait + cost table |
| `crates/app/src/remote/claude.rs` | Anthropic `/v1/messages` implementation |
| `crates/app/src/remote/keychain.rs` | `get_key(provider) / set_key / remove_key` via `keyring` |
| `crates/app/src/remote/orchestrator.rs` | `remote_chat(skill, reason, messages)` — redact → budget check → call → log |
| `crates/app/src/remote/commands.rs` | Tauri commands |
| `crates/app/src/lib.rs` | `pub mod remote;` + register ~8 commands |
| `crates/app/src/ledger/ai_review.rs` | Honor `ai.remote.enabled_for_review` setting |
| `apps/desktop/src/lib/remote/ipc.ts` | IPC wrappers + types |
| `apps/desktop/src/components/Settings/AiTab.tsx` | Add `RemoteProvidersSection` + `CallLogSection` helpers |

---

## 9. Non-goals for Landmark 2

- OpenAI, Gemini, Groq, OpenRouter providers (same trait; mechanical additions after Claude ships)
- Full 3-tier routing with per-skill overrides
- `#[ai_tool]` macro / typed tool manifest system (parent spec §5.2)
- Context recipes (YAML per intent, §5.5)
- Per-request cost estimate before calling
- Team / household shared budget (single-user assumption)
- Remote embeddings (local-only, per parent design §4.8 "Always local")

---

## 10. Open questions

1. **Anthropic API version**: use the current `2023-06-01` messages API. Upgrade when Anthropic ships a new one.
2. **Redaction of arbitrary names**: we do NOT redact names by default. Rationale: it destroys context (model can't reason about "schedule a call with Alex"). Risk: names + postcode + employer can be de-anonymizing. Mitigation: the user chooses whether remote AI is ever invoked; it's opt-in per skill.
3. **What if Anthropic returns a 529 overloaded**: retry twice with exponential backoff (250ms, 1s), then surface error to user. Log each attempt as a separate error row? → Single row with `error = "overloaded after 3 attempts"`. Simpler audit.
4. **Streaming vs non-streaming**: Landmark 2 ships non-streaming (simpler; matches Anthropic SDK default). Ledger review response is short enough this isn't user-visible lag. Streaming can be added later when a chatty consumer needs it.
5. **Do we ever bypass redaction**: no. If a skill truly needs to send raw data, that's a design failure — we should think harder about what the skill actually needs. Shipping a bypass flag invites misuse.
6. **Testing against live Anthropic**: dev tests use `wiremock` to fake responses. No live API calls in CI. Hana can smoke-test manually with her own key when the branch lands.

---

## 11. Approach summary

Ship Claude only. Property-test redaction. Audit every call. Hard-cap budget. Single gate (`ai.remote.enabled_for_review`) proves the wiring end-to-end before adding more consumers. Every future provider and every future consumer drops into the same pipeline.

---

*End of spec. Next step: implementation plan (8–10 tasks) once Hana approves.*
