# Phase 5d — Bank Sync (GoCardless) Design Spec

**Landmark:** v0.3 Ledger — bank sync
**Date:** 2026-04-17
**Status:** Design complete, awaiting user review before plan
**Supersedes (partially):** `2026-04-16-phase-5-ledger-design.md` — bank sync sections, which assumed a provider abstraction and Plaid co-habitation. This spec replaces those with a GoCardless-only implementation.

## Context

Phase 5a shipped the manual ledger core (categories, transactions, budgets). Phase 5b shipped CalDAV write-back. Phase 5c shipped recurring payments, contracts, CSV import with six bank presets, keyword-based auto-categorization, and the streaming AI month-review panel. Phase 5d — this landmark — finally connects Manor to a real bank feed.

GoCardless Bank Account Data (BAD) is a good fit for Manor's local-first model:

- Free tier for personal EU/UK use (no per-user pricing)
- User-held API credentials (BYOK): Manor stores the user's keys in Keychain alongside CalDAV passwords; no Manor-cloud middleware exists or is needed
- Read-only by API design — aligned with Manor's philosophy of never initiating payments
- Covers ~2400 institutions across the EEA and UK

## Decisions

| Question | Decision |
|---|---|
| Provider scope | GoCardless only. No `BankProvider` trait. Plaid explicitly out of scope. |
| OAuth callback | Localhost loopback (`http://127.0.0.1:<random_port>/bank-auth`). One-shot listener, 10-minute timeout, self-closing HTML response. |
| End User Agreement | `max_historical_days: 180`, `access_valid_for_days: 180`. Graceful fallback to 90/90 if the bank rejects. |
| Dedup vs Phase 5c CSV import | Soft merge on first sync only. Bank row wins; manual row's `category_id` (if bank row has none) and `note` copied over, then manual row soft-deleted. |
| Auto-categorization | Keyword categorizer runs synchronously during sync (reused from Phase 5c). Ollama batch runs lazily from LedgerView for rows still uncategorized. |
| Institution picker | Country dropdown (defaults GB) → searchable list. Per-country `/institutions` response cached 24h in SQLite. |
| Sandbox | Hidden toggle `bank_sandbox_enabled` in Settings → Advanced. Off by default. When on, the GoCardless sandbox institution appears at top of the institution list with a yellow `SANDBOX` badge. |
| Re-auth policy | GoCardless-native (180 days). The 30-day Manor-side enforcement from the original Phase 5 spec is dropped — it was written assuming a multi-provider world. |
| Pending transactions | Ignored. Only `booked` transactions are synced. Pending adds dedup complexity and gets replaced anyway. |

## 1. Architecture

### 1.1 Crate layout

```
crates/core/src/ledger/
  mod.rs                   # existing — adds bank_account and institution_cache modules
  bank_account.rs          # NEW: DAL for bank_account table
  institution_cache.rs     # NEW: 24h cache for /institutions responses

crates/app/src/ledger/
  mod.rs
  bank_commands.rs         # NEW: Tauri commands (connect, sync, disconnect, list, ...)
  gocardless.rs            # NEW: HTTP client, token rotation, OAuth flow, transaction fetch
  bank_sync.rs             # NEW: sync engine — fetch, upsert, keyword categorize, soft merge
  oauth_server.rs          # NEW: one-shot loopback HTTP listener
  bank_keychain.rs         # NEW: Keychain wrapper (mirrors caldav_keychain.rs)
```

No `BankProvider` trait. `gocardless.rs` exposes concrete functions; `bank_sync.rs` calls them directly.

### 1.2 Frontend layout

```
apps/desktop/src/
  lib/ledger/
    bank-ipc.ts              # NEW: typed invoke wrappers
    bank-state.ts            # NEW: Zustand slice for bank accounts + sync status
  components/Settings/
    BankAccountsSection.tsx  # NEW: list + connect button inside SettingsModal Accounts tab
    ConnectBankDrawer.tsx    # NEW: BYOK → country → institution picker → auth handoff
    BankAccountRow.tsx       # NEW: one connected account row with Sync / Disconnect / Reconnect
  components/Ledger/
    SyncStatusPill.tsx       # NEW: top-right of LedgerView — synced / syncing / reconnect
```

Existing `LedgerView`, `SummaryCard`, `TransactionFeed`, `MonthReviewPanel`, `AddTransactionForm`, `CsvImportDrawer`, `RecurringSection`, `ContractsSection` are unchanged.

### 1.3 First-connect data flow

```
User taps "Connect bank"
  → ConnectBankDrawer opens (BYOK wizard if credentials not saved)
  → ledger_bank_list_institutions(country="GB")          # cached 24h
  → user picks Barclays
  → ledger_bank_begin_connect(institution_id)
      ├─ start loopback listener on 127.0.0.1:0 (OS-assigned port)
      ├─ POST /api/v2/agreements/enduser/
      │    { institution_id, max_historical_days: 180, access_valid_for_days: 180 }
      │    — on 400, retry with 90/90
      ├─ POST /api/v2/requisitions/
      │    { institution_id, agreement, redirect: "http://127.0.0.1:<port>/bank-auth",
      │      reference: <uuid> }
      └─ return { auth_url, requisition_id, reference }
  → frontend opens auth_url via @tauri-apps/plugin-shell::open()
  → user logs into bank in external browser
  → GoCardless redirects to http://127.0.0.1:<port>/bank-auth?ref=<uuid>
  → loopback listener:
      ├─ serves self-closing HTML (800ms window.close())
      ├─ sends callback params to bank_commands via oneshot channel
      └─ shuts down
  → ledger_bank_complete_connect(requisition_id, reference)
      ├─ GET /api/v2/requisitions/{id}/  → list of account external_ids
      ├─ for each account:
      │    GET /api/v2/accounts/{external_id}/details/  → institution + account names
      │    INSERT bank_account row with institution_id, institution_logo_url,
      │    requisition_expires_at, reference, initial_sync_completed_at=NULL
      └─ trigger first sync in background
  → drawer shows "Syncing 180 days…" progress, then "✓ Connected Barclays"
```

### 1.4 Ongoing sync data flow

Runs from the existing CalDAV/rhythm scheduler on a 6h tick. For each non-deleted, non-paused `bank_account`:

1. **Preflight.** If `requisition_expires_at < now()`, set `sync_paused_reason = 'requisition_expired'`, insert `proposal` row with `kind = 'bank_reconnect'` (deduped by `reference_id = bank_account_id`), skip.
2. **Ensure bearer token.** `gocardless::ensure_access_token()` — read from Keychain, refresh via `/token/refresh/` if stale, re-auth via `/token/new/` if refresh also stale.
3. **Fetch.** `GET /api/v2/accounts/{external_id}/transactions/?date_from=<yyyy-mm-dd>`. On first sync (`last_synced_at IS NULL`), `date_from = now - max_historical_days_granted`. On subsequent syncs, `date_from = max(last_synced_at - 3 days, requisition_created_at)`. The 3-day overlap catches banks that post transactions with delay. Only the `booked` array — ignore `pending`.
4. **Map + upsert.** For each raw transaction: map to `ledger_transaction` shape, `INSERT … ON CONFLICT (bank_account_id, external_id) DO NOTHING`. Provider-enriched merchant goes to `merchant`; raw description goes to `description`; `source = 'sync'`.
5. **Keyword categorize.** For any inserted row with `category_id IS NULL`, run the Phase 5c keyword categorizer synchronously.
6. **First-sync soft merge.** If `initial_sync_completed_at IS NULL`, run the dedup pass (§4.3). Set `initial_sync_completed_at = now()`.
7. **Finalize.** Update `last_synced_at = now()`, `sync_paused_reason = NULL`.

### 1.5 Integration points

- **Phase 5c MonthReviewPanel** — unchanged. Bank-synced rows land in `ledger_transaction` and aggregate automatically.
- **Phase 5c keyword categorizer** — promoted from CSV-only to a shared helper. `bank_sync.rs` imports and reuses it.
- **Assistant proposal / bubble pipeline** — bank sync inserts `proposal` rows with `kind = 'bank_reconnect'`, `'bank_sync_failed'`, or `'budget_nudge'`.
- **Settings → Accounts tab** — CalDAV section unchanged; bank accounts section added below.
- **Sidebar** — no changes (Ledger icon already exists from 5a).

## 2. Database (V13__bank_sync.sql)

```sql
-- Phase 5d: GoCardless bank account data integration.

-- Extend bank_account stub (from V5) with GoCardless-specific fields.
ALTER TABLE bank_account ADD COLUMN institution_id              TEXT;
ALTER TABLE bank_account ADD COLUMN institution_logo_url        TEXT;
ALTER TABLE bank_account ADD COLUMN reference                   TEXT;   -- UUID sent to /requisitions
ALTER TABLE bank_account ADD COLUMN requisition_created_at      INTEGER; -- unix seconds, set on /requisitions success
ALTER TABLE bank_account ADD COLUMN max_historical_days_granted INTEGER; -- 180 or 90, from EUA that succeeded
ALTER TABLE bank_account ADD COLUMN sync_paused_reason          TEXT;   -- 'requisition_expired' | NULL
ALTER TABLE bank_account ADD COLUMN initial_sync_completed_at   INTEGER;

-- Rename for accuracy — GoCardless lifetime is requisition-bound, not token-bound.
ALTER TABLE bank_account RENAME COLUMN token_expires_at TO requisition_expires_at;

-- 24h cache for /institutions responses, per country.
CREATE TABLE gocardless_institution_cache (
    country                TEXT    NOT NULL,    -- 'GB', 'FR', 'DE', 'IE', 'ES', ...
    institution_id         TEXT    NOT NULL,
    name                   TEXT    NOT NULL,
    bic                    TEXT,
    logo_url               TEXT,
    max_historical_days    INTEGER NOT NULL,    -- bank-advertised ceiling
    access_valid_for_days  INTEGER NOT NULL,    -- bank-advertised ceiling
    fetched_at             INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (country, institution_id)
);

CREATE INDEX idx_gocardless_institution_cache_fetched
    ON gocardless_institution_cache(fetched_at);

-- Sandbox toggle — hidden in Settings → Advanced, off by default.
INSERT OR IGNORE INTO setting (key, value) VALUES ('bank_sandbox_enabled', 'false');
```

What is **not** in the schema and why:

- **No `access_token` / `refresh_token` columns.** Both live in Keychain.
- **No `secret_id` / `secret_key` columns.** User's GoCardless API credentials are per-install (BYOK) and live in Keychain.
- **No separate `bank_sync_status` table.** `bank_account.last_synced_at` + `sync_paused_reason` covers UI needs.
- **No change to `ledger_transaction`.** `bank_account_id`, `external_id`, `source`, `UNIQUE(bank_account_id, external_id)` are already present from V5.
- **No Plaid columns.** GoCardless-only.

### 2.1 Keychain layout

All entries are `kSecClassGenericPassword` under service `com.hanamorix.manor.gocardless`:

| Account | Value | Lifetime |
|---|---|---|
| `secret_id` | user's GoCardless Secret ID (from BYOK wizard) | until disconnect-last-account |
| `secret_key` | user's GoCardless Secret Key | until disconnect-last-account |
| `access_token` | bearer, rotated automatically | 24h |
| `refresh_token` | refresh token | 30d |

All four entries wiped when the last `bank_account` is disconnected.

Per-account `requisition_id` stays in the DB — it is an opaque handle, not a secret.

## 3. Tauri commands

All commands live in `crates/app/src/ledger/bank_commands.rs`.

| Command | Purpose |
|---|---|
| `ledger_bank_credentials_status` | Returns whether `secret_id` + `secret_key` exist in Keychain. Drives BYOK wizard vs direct connect. |
| `ledger_bank_save_credentials` | Save `secret_id` + `secret_key` to Keychain, verify with `POST /token/new/`, return success or structured error. |
| `ledger_bank_list_institutions` | Args: `country`. Returns cached list (or fetches + caches). Prepends sandbox institution if `bank_sandbox_enabled = true`. |
| `ledger_bank_begin_connect` | Args: `institution_id`. Creates EUA (180/180, fallback 90/90), creates requisition, starts loopback listener on a random port, returns `{ auth_url, requisition_id, reference }`. Frontend opens `auth_url` via `@tauri-apps/plugin-shell::open()`. |
| `ledger_bank_complete_connect` | Called from the loopback handler. Args: `requisition_id`, `reference`. Fetches account list, inserts `bank_account` rows, kicks off first sync. Returns inserted account IDs. |
| `ledger_bank_list_accounts` | All non-deleted `bank_account` rows with live `sync_status` and `requisition_expires_at`. |
| `ledger_bank_sync_now` | Args: `bank_account_id` (optional; null = all). Triggers sync on demand. |
| `ledger_bank_disconnect` | Args: `bank_account_id`. Calls `DELETE /api/v2/requisitions/{id}/`, soft-deletes the row. If it was the last account, wipes all Keychain entries for the service. |
| `ledger_bank_reconnect` | Args: `bank_account_id`. Shortcut for expired requisitions — reuses `institution_id`, runs begin/complete_connect again, replaces the row on success. |
| `ledger_bank_autocat_pending` | Debounced from LedgerView open. Fetches uncategorized bank-synced rows < 7 days old, batches to Ollama (T1 Haiku), updates categories. |

## 4. Implementation details

### 4.1 OAuth loopback (`oauth_server.rs`)

Spawned on a dedicated OS thread (not tokio — `tiny_http` is blocking). Binds `127.0.0.1:0`, lets the OS pick the port, returns the port and a `oneshot::Receiver<HashMap<String, String>>` for the callback params.

Timeout: 10 minutes. If no callback received, the thread exits and the channel drops, surfacing as `BankError::OAuthTimeout` to the frontend.

Serves one response, then shuts down:

- `GET /bank-auth?...` → 200, `Content-Type: text/html`, self-closing HTML body (below), sends params, shuts down.
- Anything else → 404, keeps listening.

The self-closing HTML:

```html
<!doctype html>
<html><head><title>Manor</title></head>
<body style="font-family:system-ui;background:#1a1a2e;color:#e4e4e7;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
<div style="text-align:center">
  <h1>Connected.</h1>
  <p>You can close this tab — Manor has taken over.</p>
</div>
<script>setTimeout(() => window.close(), 800);</script>
</body></html>
```

The 800ms delay lets the user register the confirmation before the tab closes. If `window.close()` is blocked (some browsers block tabs not opened by script), the message still reads as a terminal state.

### 4.2 GoCardless client (`gocardless.rs`)

Single `reqwest::Client` with `User-Agent: Manor/<version>`. Base URL: `https://bankaccountdata.gocardless.com`.

Public functions:

```rust
pub async fn test_credentials(secret_id: &str, secret_key: &str) -> Result<()>;
pub async fn ensure_access_token() -> Result<String>;   // returns bearer, rotates transparently
pub async fn list_institutions(country: &str) -> Result<Vec<RawInstitution>>;
pub async fn create_agreement(institution_id: &str, days: (u16, u16)) -> Result<String>;
pub async fn create_requisition(institution_id: &str, agreement_id: &str, redirect: &str, reference: &str) -> Result<RawRequisition>;
pub async fn fetch_requisition_accounts(requisition_id: &str) -> Result<Vec<String>>;
pub async fn fetch_account_details(external_id: &str) -> Result<RawAccountDetails>;
pub async fn fetch_transactions(external_id: &str, date_from: &str) -> Result<Vec<RawTransaction>>;
pub async fn delete_requisition(requisition_id: &str) -> Result<()>;
```

Error mapping (`BankError`):

| HTTP | Mapped to |
|---|---|
| 401 (tokens) | `AuthFailed` — triggers token refresh chain |
| 400 on `/agreements/enduser/` with "max_historical_days exceeds" | `EuaTooLong` — caller retries with 90/90 |
| 409 on `/transactions/` with "requisition expired" | `RequisitionExpired` — caller sets `sync_paused_reason` |
| 429 | `RateLimited(retry_after)` — caller defers to next tick |
| 5xx | `UpstreamTransient` — retry with exponential backoff (3 attempts) |
| anything else | `Other(status, body)` — surfaced verbatim |

### 4.3 First-sync soft merge (`bank_sync::soft_merge`)

Runs once per account, when `initial_sync_completed_at IS NULL` after step 5 of the sync flow.

```sql
-- Find manual rows within the sync window that match a bank-synced row.
SELECT m.id AS manual_id, m.category_id, m.note,
       b.id AS bank_id, b.category_id AS bank_category_id
FROM ledger_transaction m
JOIN ledger_transaction b ON (
      b.bank_account_id = ?account_id
  AND b.source = 'sync'
  AND m.bank_account_id IS NULL
  AND m.source = 'manual'
  AND m.amount_pence = b.amount_pence
  AND ABS(m.date - b.date) <= 86400     -- ±1 day
)
WHERE m.deleted_at IS NULL AND b.deleted_at IS NULL;
```

For each pair:

- If `bank_category_id IS NULL`, set `bank.category_id = manual.category_id`.
- Append `manual.note` to `bank.note` (if `bank.note IS NULL`, just set; otherwise concatenate with `\n`).
- Soft-delete `manual` (`deleted_at = unixepoch()`).

This only runs on first sync per account — subsequent syncs dedup purely on `(bank_account_id, external_id)`.

### 4.4 Rate limiting

GoCardless BAD enforces **4 requests per 24h per endpoint per resource** for most endpoints, including `/transactions/`. The scheduler's 6h tick gives us 4 syncs/day — exactly at the ceiling.

`bank_sync.rs` defends against accidental double-sync: any account with `last_synced_at > now() - 5h` is skipped in the background tick. The user's manual `ledger_bank_sync_now` bypasses this check but surfaces a warning if it would blow the budget.

### 4.5 Lazy Ollama autocat

Triggered from the frontend when:

- LedgerView mounts (debounced 2s)
- MonthReviewPanel computes summary

Backend behaviour (`ledger_bank_autocat_pending`):

1. Query uncategorized bank-synced rows created < 7 days ago, limit 50.
2. If empty, return `{ processed: 0 }`.
3. Build one Ollama prompt listing each transaction's `(description, merchant, amount)` and the category names. Model: T1 Haiku via the existing remote orchestrator.
4. Parse the model's `{id: category_name}` mapping, update rows.
5. Return `{ processed: N }`.

Offline-safe: if the remote orchestrator is unreachable, the call is a no-op; rows stay "Other" until next open.

## 5. UI

### 5.1 Settings → Accounts — Bank Accounts section

Appears below the existing CalDAV section:

```
─── Bank Accounts ─────────────────────────────────
                                       [+ Connect]

  ┌────────────────────────────────────────────┐
  │ [logo] Barclays Current                     │
  │        synced 2h ago · expires in 162 days  │
  │                            [↻ Sync]  [✕]   │
  └────────────────────────────────────────────┘
```

Expired state: row turns amber; `[↻ Sync]` and `[✕]` replaced with `[Reconnect]`.

### 5.2 BYOK wizard (first connect only)

When the user taps `[+ Connect]` and `ledger_bank_credentials_status` is `false`, the drawer opens in wizard mode:

```
  Connect a bank

  Manor connects to your bank through GoCardless, a free
  EU/UK service. You'll need a GoCardless account and API
  keys. Takes about 3 minutes, one time.

  1. Create a free account → [open bankaccountdata.gocardless.com]
  2. Go to User Secrets → copy your Secret ID and Secret Key
  3. Paste them below.

  Secret ID     [_______________________________]
  Secret Key    [_______________________________]

  Your keys are stored in macOS Keychain. They never leave
  this device.

                                      [Cancel]  [Continue]
```

On **Continue**: `ledger_bank_save_credentials` stores and verifies with `POST /token/new/`. On success the drawer transitions (no reload) to the institution picker.

### 5.3 Institution picker

```
  Connect a bank

  Country   [▼ United Kingdom          ]

  Search    [🔍 Type to filter…        ]

  ┌──────────────────────────────────┐
  │ [logo] Barclays                   │
  │ [logo] Bank of Scotland           │
  │ [logo] First Direct               │
  │ [logo] HSBC                       │
  │ [logo] Lloyds                     │
  │ [logo] Monzo                      │
  │ [logo] Nationwide                 │
  │ [logo] NatWest                    │
  │ [logo] Revolut                    │
  │ [logo] Santander                  │
  │ [logo] Starling                   │
  │ [logo] TSB                        │
  │  …                                │
  └──────────────────────────────────┘
```

Country defaults to GB (Manor install locale). Changing country re-fetches (or reads cache for) that country's list. Sandbox institution appears at top with a yellow `SANDBOX` badge when `bank_sandbox_enabled = true`.

After selection: browser opens → user authorises → loopback → `ledger_bank_complete_connect` → drawer shows a success state with first-sync progress:

```
  ✓ Connected Barclays

  Syncing 180 days of transactions… (47%)

                                         [Done]
```

Progress % comes from Tauri events `ledger_bank_sync_progress { account_id, done, total }` emitted by the sync engine in batches of 100.

### 5.4 LedgerView — SyncStatusPill

Tiny right-aligned pill above SummaryCard:

- **Synced**: `✓ synced 2h ago` — pale green
- **Syncing**: `⟳ syncing…` — animated
- **Paused (expired)**: `⚠ reconnect Barclays` — amber, clickable → Settings → Accounts
- **No accounts connected**: hidden

### 5.5 Assistant bubble nudges

| `proposal.kind` | Trigger | Bubble text |
|---|---|---|
| `bank_reconnect` | Requisition expired OR <7 days to expiry — once per account per expiry cycle | *"Your Barclays link renews in N days — want to reconnect now?"* (with tappable Reconnect action) |
| `bank_sync_failed` | 3 consecutive sync failures on the same account, 24h window | *"Barclays sync has failed three times today. Something might be up with GoCardless — want me to retry?"* |
| `budget_nudge` | Already from Phase 5 spec — unchanged here | *"Eating Out is £14 over this month…"* |

Dedup key: `(kind, reference_id = bank_account_id, cycle)` where `cycle = month` for `budget_nudge`, `requisition_expires_at` epoch for `bank_reconnect`, `day` for `bank_sync_failed`.

### 5.6 Settings → Advanced — sandbox toggle

```
  ─── Developer ─────
  [ ] Enable GoCardless sandbox institution

      When on, the institution picker includes a
      SANDBOX test bank that returns deterministic fake
      transactions. For development only.
```

## 6. Dependencies

| Crate | Version | Why |
|---|---|---|
| `reqwest` | already used | HTTP + TLS |
| `tiny_http` | `~0.12` | One-shot loopback listener — synchronous, tiny, spawned on its own thread |
| `url` | already used | Building redirect URLs with port |
| `uuid` | already used | `reference` UUIDs |
| `keyring` | already used | Keychain |
| `tauri-plugin-shell` | already used | Opening auth URLs |
| `wiremock` | dev-dep | HTTP mocking in integration tests |

No new frontend deps.

## 7. Testing strategy

**Unit tests (core, in-memory SQLite):**

- `bank_account.rs` — insert/list/soft-delete, column rename migration correctness
- `institution_cache.rs` — 24h TTL staleness, per-country isolation

**Unit tests (app):**

- `gocardless.rs` — token rotation chain (access stale → refresh; refresh stale → re-auth); pagination; `booked` vs `pending` filter; error mapping for 401/400/409/429/5xx
- `bank_sync.rs` — dedup via `UNIQUE(bank_account_id, external_id)`; soft-merge preserves manual `note` and `category_id`; rate-limit skip when `last_synced_at` > now-5h
- `oauth_server.rs` — binds, receives callback, times out after 10 min; rejects malformed params; serves correct `Content-Type`

**Integration test (app, wiremock):**

- `tests/bank_sync_integration.rs` — full happy path: credentials → institutions → agreement → requisition → complete_connect → first sync → second sync with 3-day overlap. Uses GoCardless sandbox response fixtures.

**Manual acceptance tests (Hana's real Barclays):**

1. Fresh install, no bank accounts.
2. Settings → Connect bank → BYOK wizard → verify keys stored.
3. Pick GB → Barclays → browser opens → authorise → tab shows "Connected." and closes → Manor foregrounds.
4. First sync runs, progress bar counts up, 180 days of real transactions appear in LedgerView.
5. Verify SummaryCard totals, budget bar, categories auto-filled by keyword categorizer.
6. Pre-sync CSV-imported rows: verify soft-merge preserved their categories on the bank-synced rows and removed the manual duplicates.
7. Force-expire requisition in DB (`UPDATE bank_account SET requisition_expires_at = unixepoch() WHERE id = …`); verify amber SyncStatusPill and `bank_reconnect` bubble.
8. Disconnect account; verify Keychain entries wiped (`security find-generic-password -s com.hanamorix.manor.gocardless` returns nothing).

## 8. Out of scope

- **Windows / Linux.** Keychain dependency.
- **Plaid.** Ever.
- **Payment initiation.** GoCardless BAD is read-only by API design.
- **Multi-currency rollups.** Non-GBP transactions store native amount + symbol; `SummaryCard` totals and budgets remain GBP-only. Conversion is a future landmark.
- **Statement PDF parsing.** CSV covers offline ingestion.
- **Brokerage / crypto exchanges.** Not a GoCardless product.
- **Mobile / iOS.**
- **Shared household budgets.**
- **Webhook-driven sync.** GoCardless BAD doesn't offer retail-account webhooks; polling only.

## 9. Acceptance criteria

Phase 5d is complete when:

- [ ] Hana can connect Barclays end-to-end with her real GoCardless free-tier account.
- [ ] 180 days of real transactions land in LedgerView with keyword-based categories filled in.
- [ ] MonthReviewPanel generates its AI narrative over bank-synced data without modification.
- [ ] CSV-imported historical transactions are soft-merged on first sync with `category_id` and `note` preserved.
- [ ] Requisition expiry triggers a `bank_reconnect` bubble and an amber SyncStatusPill.
- [ ] Disconnecting the last account wipes all GoCardless entries from Keychain.
- [ ] Unit + integration tests pass; `cargo fmt` + `cargo clippy -- -D warnings` clean.
- [ ] Sandbox toggle works; no sandbox tile visible in production UI.
