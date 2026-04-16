# v0.3 Ledger — Design Spec

## Goal

Add a Ledger view to Manor: read-only bank sync (GoCardless for EU/UK, Plaid for US) plus manual transaction entry, per-category budgets, and Ollama-powered budget proposals. 30-day token lifetime with proactive re-auth nudges via the existing avatar bubble system.

## Decisions Made

| Question | Decision |
|---|---|
| Bank sync provider | Provider abstraction — GoCardless (EU/UK) + Plaid (US), user picks region |
| API key setup | In-app guided signup wizard (GoCardless free tier / Plaid Development) + paste-your-own-key for advanced users. No credentials bundled in source (AGPL). |
| Token expiry UX | Manor enforces 30-day re-auth cycle regardless of provider lifetime. Avatar nudges 7 days before, silent banner fallback at expiry. |
| Manual entry | Yes — manual + sync coexist in the same transaction feed |
| Categories | Fixed defaults + user-editable (rename, add, delete, merge) |
| Budgets in v0.3 | Yes — monthly limits per category, progress bars, over-budget proposals |
| Layout | Dark gradient summary card + budget health badges + day-grouped transaction feed |

---

## 1. Architecture

### 1.1 Provider abstraction

A `BankProvider` trait in `crates/core` defines the interface. Both provider implementations live in `crates/app` (they need HTTP + Keychain access). The rest of the app — transaction store, sync engine, TTL logic, Tauri commands — is provider-agnostic.

```
crates/core/src/ledger/
  mod.rs
  account.rs        — bank_account DAL
  transaction.rs    — transaction DAL
  category.rs       — category DAL + defaults seeding
  budget.rs         — budget DAL + rollup queries
  provider.rs       — BankProvider trait definition

crates/app/src/ledger/
  mod.rs
  commands.rs       — Tauri commands (thin glue)
  sync.rs           — sync engine: poll provider, dedup, persist
  gocardless.rs     — GoCardless Bank Account Data impl
  plaid.rs          — Plaid impl
  token_monitor.rs  — background token-expiry watcher

apps/desktop/src/
  lib/ledger/
    ipc.ts          — typed Tauri invoke wrappers
    state.ts        — Zustand store
  components/Ledger/
    LedgerView.tsx  — main view
    SummaryCard.tsx — dark gradient card + badges
    TransactionFeed.tsx — day-grouped list
    TransactionRow.tsx  — single row with emoji icon
    BudgetSheet.tsx — budget management drawer
    AddTransactionForm.tsx — manual entry drawer
```

### 1.2 OAuth callback (deep link)

Both GoCardless and Plaid require the user to authorise in a browser. Tauri 2's `tauri-plugin-deep-link` registers the `manor://` custom URL scheme. The OAuth redirect target is `manor://bank-auth?provider=gocardless&ref=<requisition_id>` (or Plaid's equivalent). Tauri intercepts the URL and completes the auth flow without any server infrastructure.

### 1.3 API key setup

Manor is AGPL open source — no credentials are bundled in source. Instead, first-run setup is an in-app guided wizard:

1. User opens Settings → Bank Accounts → Connect
2. Manor shows a one-screen wizard: pick region (UK/EU → GoCardless, US → Plaid)
3. For GoCardless: wizard links to the GoCardless free signup page (genuinely free for personal use), then prompts the user to paste their `secret_id` and `secret_key` — stored in Keychain, never in source
4. For Plaid: same pattern with Plaid Development credentials (free tier)
5. Advanced users can swap keys at any time via Settings → Bank Accounts → Manage Keys

The wizard is designed to feel like connecting a service (guided, in-app, one page) not like developer setup.

### 1.4 Token storage

Provider access tokens and refresh tokens are stored in macOS Keychain under service `com.hanamorix.manor.bank.<provider>.<account_id>` — identical pattern to CalDAV passwords. The `bank_account` table stores metadata and `token_expires_at` (unix seconds) but never the token itself.

### 1.4 Background token monitor

`token_monitor.rs` runs on a 1-hour tick (same scheduler as CalDAV sync). Manor enforces a **30-day re-auth cycle** as a UX policy — `token_expires_at` is set to `connected_at + 30 days` regardless of the provider's actual token lifetime (GoCardless requisitions last 90 days; Plaid access tokens vary by institution). This keeps the re-auth cadence predictable for the user.

For each connected account: if `token_expires_at` is within 7 days and `last_nudge_at` is not set this cycle, fire an assistant bubble: *"Your Barclays link refreshes in N days — want to reconnect now?"*. At expiry, sync pauses and a banner appears in LedgerView.

---

## 2. Database Schema (V5__ledger.sql)

```sql
-- Category — fixed defaults + user-editable
CREATE TABLE category (
  id          INTEGER PRIMARY KEY,
  name        TEXT    NOT NULL,
  emoji       TEXT    NOT NULL DEFAULT '💳',
  is_income   INTEGER NOT NULL DEFAULT 0,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  is_default  INTEGER NOT NULL DEFAULT 0,  -- seeded rows, cannot delete
  deleted_at  INTEGER
);

-- Default seed rows (inserted by migration)
-- id 1  Groceries       🛒  income=0
-- id 2  Eating Out      🍕  income=0
-- id 3  Transport       🚇  income=0
-- id 4  Utilities       ⚡  income=0
-- id 5  Subscriptions   📱  income=0
-- id 6  Health          💊  income=0
-- id 7  Shopping        🛍️  income=0
-- id 8  Entertainment   🎬  income=0
-- id 9  Other           💳  income=0
-- id 10 Income          💼  income=1

-- Connected bank account
CREATE TABLE bank_account (
  id               INTEGER PRIMARY KEY,
  provider         TEXT    NOT NULL,  -- 'gocardless' | 'plaid'
  institution_name TEXT    NOT NULL,
  account_name     TEXT    NOT NULL,
  account_type     TEXT    NOT NULL DEFAULT 'current',  -- current|savings|credit
  currency         TEXT    NOT NULL DEFAULT 'GBP',
  external_id      TEXT    NOT NULL,  -- provider's account id
  requisition_id   TEXT,              -- GoCardless: requisition id
  token_expires_at INTEGER,           -- unix seconds
  last_synced_at   INTEGER,
  last_nudge_at    INTEGER,           -- prevent repeat nudges same cycle
  created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
  deleted_at       INTEGER
);

-- Transaction (synced or manual)
CREATE TABLE ledger_transaction (
  id              INTEGER PRIMARY KEY,
  bank_account_id INTEGER REFERENCES bank_account(id),  -- NULL = manual
  external_id     TEXT,            -- provider tx id (dedup key)
  amount_pence    INTEGER NOT NULL, -- negative = out, positive = in
  currency        TEXT    NOT NULL DEFAULT 'GBP',
  description     TEXT    NOT NULL, -- raw from provider or user input
  merchant        TEXT,             -- cleaned name (provider-enriched or user-set)
  category_id     INTEGER REFERENCES category(id),
  date            INTEGER NOT NULL, -- unix seconds (local midnight)
  source          TEXT    NOT NULL DEFAULT 'manual',  -- 'manual' | 'sync'
  note            TEXT,
  created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
  deleted_at      INTEGER,
  UNIQUE(bank_account_id, external_id)  -- dedup on sync
);

CREATE INDEX idx_ledger_transaction_date ON ledger_transaction(date);
CREATE INDEX idx_ledger_transaction_category ON ledger_transaction(category_id);

-- Monthly budget per category
CREATE TABLE budget (
  id          INTEGER PRIMARY KEY,
  category_id INTEGER NOT NULL REFERENCES category(id),
  amount_pence INTEGER NOT NULL,  -- monthly limit in pence
  created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
  deleted_at  INTEGER,
  UNIQUE(category_id)
);
```

---

## 3. BankProvider Trait

```rust
// crates/core/src/ledger/provider.rs

pub struct RawTransaction {
    pub external_id: String,
    pub amount_pence: i64,    // negative = debit
    pub currency: String,
    pub description: String,
    pub merchant: Option<String>,
    pub date: i64,            // unix seconds
}

pub struct LinkedAccount {
    pub external_id: String,
    pub institution_name: String,
    pub account_name: String,
    pub account_type: String,
    pub currency: String,
}

pub trait BankProvider: Send + Sync {
    /// Returns the OAuth URL the user must visit to authorise.
    fn auth_url(&self, redirect_uri: &str) -> anyhow::Result<String>;

    /// Called when Tauri intercepts the deep-link callback.
    /// Exchanges the code/ref for tokens, stores in Keychain,
    /// returns the list of accounts available under this connection.
    async fn complete_auth(
        &self,
        callback_params: &std::collections::HashMap<String, String>,
        redirect_uri: &str,
    ) -> anyhow::Result<(Vec<LinkedAccount>, i64 /* token_expires_at */)>;

    /// Fetch transactions for one account since `since_ts`.
    async fn fetch_transactions(
        &self,
        account_external_id: &str,
        since_ts: i64,
    ) -> anyhow::Result<Vec<RawTransaction>>;

    /// Refresh the access token using the stored refresh token.
    /// Updates Keychain and returns the new `token_expires_at`.
    async fn refresh_token(&self, account_external_id: &str) -> anyhow::Result<i64>;
}
```

---

## 4. Tauri Commands

| Command | Description |
|---|---|
| `ledger_list_accounts` | List connected bank accounts |
| `ledger_connect_account` | Start OAuth flow — returns auth URL to open |
| `ledger_complete_auth` | Called from deep-link handler with callback params |
| `ledger_disconnect_account` | Soft-delete account + revoke Keychain token |
| `ledger_sync_now` | Manual sync trigger for one account |
| `ledger_list_transactions` | Paginated list with date range + category filter |
| `ledger_add_transaction` | Manual entry |
| `ledger_update_transaction` | Edit description / category / note |
| `ledger_delete_transaction` | Soft-delete |
| `ledger_list_categories` | All non-deleted categories |
| `ledger_upsert_category` | Create or rename a category |
| `ledger_delete_category` | Soft-delete (only non-default, reassign txns to Other) |
| `ledger_list_budgets` | All budgets |
| `ledger_upsert_budget` | Set or update monthly limit for a category |
| `ledger_delete_budget` | Remove budget for a category |
| `ledger_monthly_summary` | Aggregate: total in/out + per-category spend vs budget |

---

## 5. Frontend

### 5.1 Ledger nav icon

A `💰` icon added to the Sidebar between TimeBlocks and the spacer. `View` type in `nav.ts` gains `"ledger"`.

### 5.2 LedgerView

Top-level view following the same stacked-cards pattern as Today, Chores, TimeBlocks:

```
<SummaryCard />        — dark gradient, monthly totals, budget health badges
<TransactionFeed />    — day-grouped rows, infinite scroll (30 days default)
```

A `+ Add` FAB sits above the avatar footprint. Tapping it opens `AddTransactionForm` (slide-in drawer).

### 5.3 SummaryCard

Dark gradient (`#1a1a2e → #16213e`) card showing:
- Month label (e.g. "April 2026")
- Total spent (large, bold)
- Budget denominator + remaining ("of £2,000 · £760 remaining")
- Progress bar — green at <75%, amber at 75–99%, red at 100%+
- Badge row — "✓ On track", "⚠️ Category 95%", "🔴 Category over" — one badge per alerting category, max 3

Gradient tint shifts: normal → dark blue, near limit → dark amber, over → dark red.

### 5.4 TransactionRow

```
[emoji icon]  Merchant name          -£12.40
              Category · Account
```

Emoji icon background colour is derived from category (Groceries = light blue, Eating Out = light red, Income = light green, etc.). Income rows show amount in green.

### 5.5 Avatar budget nudges

When `token_monitor` or `sync.rs` detects a budget condition, it inserts a `proposal` row with `kind = 'budget_nudge'`. The existing proposal-to-bubble pipeline surfaces it as an assistant bubble:

- **Near limit (75–99%)**: *"You've used 89% of your Eating Out budget with 12 days left — want me to suggest an adjustment?"*
- **Over budget**: *"Eating Out is £14 over this month. Want to review next month's limit?"*
- **Token expiring**: *"Your Barclays link refreshes in 7 days — want to reconnect now?"*

Proposals fire at most once per category per month (de-duped by `proposal` table's `kind + reference_id`).

### 5.6 Manual entry

Pill command: `/spent £12.50 coffee` — creates a manual transaction. Ollama auto-categorises from the description using a lightweight tool call. User can override category from the transaction row.

`AddTransactionForm` drawer fields: amount, description, category (picker), date (defaults today), note (optional).

### 5.7 Settings — Accounts tab (bank section)

New section within existing SettingsModal Accounts tab (below CalDAV accounts):

```
Bank Accounts
  [+ Connect bank]

  ┌─────────────────────────────────┐
  │ 🏦 Barclays Current             │
  │ GoCardless · last synced 2h ago │
  │ Token expires in 24 days        │
  │                    [Sync] [✕]   │
  └─────────────────────────────────┘
```

BYOK: a disclosure section "Use your own API key" with provider-specific instructions and a text field, mirroring CalDAV's app-password pattern.

---

## 6. Sync Engine

`sync.rs` runs on app launch and on a 6-hour background tick:

1. For each non-deleted `bank_account`:
   a. Check `token_expires_at` — if expired, mark account as `sync_paused`, skip
   b. Call `provider.fetch_transactions(account_id, last_synced_at)`
   c. For each raw transaction: upsert into `ledger_transaction` using `(bank_account_id, external_id)` as dedup key
   d. Auto-categorise uncategorised transactions via a single Ollama batch call (T1 Haiku)
   e. Update `last_synced_at`
2. Recompute monthly summary, check budget thresholds, fire proposals if needed

---

## 7. Default Categories (seed data)

| # | Name | Emoji | Income |
|---|---|---|---|
| 1 | Groceries | 🛒 | No |
| 2 | Eating Out | 🍕 | No |
| 3 | Transport | 🚇 | No |
| 4 | Utilities | ⚡ | No |
| 5 | Subscriptions | 📱 | No |
| 6 | Health | 💊 | No |
| 7 | Shopping | 🛍️ | No |
| 8 | Entertainment | 🎬 | No |
| 9 | Other | 💳 | No |
| 10 | Income | 💼 | Yes |

Default categories (`is_default = 1`) cannot be deleted — only renamed.

---

## 8. What's Explicitly Out of Scope (v0.3)

- Outgoing bank transfers / payments (read-only, no write access ever)
- Multi-currency conversion
- Receipt / photo attachment (v0.4+)
- Shared household budgets (v0.4+)
- CSV / OFX import
- Windows / Linux bank sync (macOS Keychain dependency)
- Contracts / subscriptions tracking (uses `recurring` table, deferred to v0.4)
