# Phase 5c: Ledger Completions — Design Spec

- **Date**: 2026-04-16
- **Status**: Approved
- **Authors**: Hana (product), Nell (architect/engineer)

## Goal

Close the v0.3 Ledger promise: recurring payments with auto-insertion, contracts with renewal alerts, CSV import from named bank presets, and a persistent month-in-review panel with on-demand AI narrative.

---

## 1. Data Model (V7 Migration)

### 1.1 `recurring_payment`

```sql
CREATE TABLE recurring_payment (
    id               INTEGER PRIMARY KEY,
    description      TEXT    NOT NULL,
    amount_pence     INTEGER NOT NULL,
    currency         TEXT    NOT NULL DEFAULT 'GBP',
    category_id      INTEGER REFERENCES category(id),
    day_of_month     INTEGER NOT NULL CHECK (day_of_month BETWEEN 1 AND 28),
    active           INTEGER NOT NULL DEFAULT 1,
    note             TEXT,
    created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at       INTEGER
);
```

`day_of_month` is capped at 28 (safe for February). Payments on the 29th/30th/31st are modelled as the 28th.

### 1.2 `ledger_transaction` — new column

```sql
ALTER TABLE ledger_transaction ADD COLUMN recurring_payment_id INTEGER REFERENCES recurring_payment(id);
```

Allows tracing which template generated a given transaction. `source = 'recurring'` on auto-inserted rows.

### 1.3 `contract`

```sql
CREATE TABLE contract (
    id                    INTEGER PRIMARY KEY,
    provider              TEXT    NOT NULL,
    kind                  TEXT    NOT NULL DEFAULT 'other',
    description           TEXT,
    monthly_cost_pence    INTEGER NOT NULL,
    term_start            INTEGER NOT NULL,
    term_end              INTEGER NOT NULL,
    exit_fee_pence        INTEGER,
    renewal_alert_days    INTEGER NOT NULL DEFAULT 30,
    recurring_payment_id  INTEGER REFERENCES recurring_payment(id),
    note                  TEXT,
    created_at            INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at            INTEGER
);
```

`kind` values: `phone | broadband | insurance | energy | other`.

---

## 2. Auto-Insert Logic

On every app open, `recurring_payment::auto_insert_due` runs:

1. For each active recurring payment, check whether a transaction with that `recurring_payment_id` already exists for the current calendar month (year + month match on `date`).
2. If today's day-of-month ≥ `day_of_month` and no transaction exists yet: insert one with `source = 'recurring'`, `recurring_payment_id` set, `date = today midnight UTC`.
3. Wrapped in a single transaction — all-or-nothing per app open.

This runs in `lib.rs` `setup()` after the existing background sync spawn, on the blocking thread.

---

## 3. Contract Renewal Alerts

On every app open, `contract::check_renewals` runs after auto-insert:

- Returns all contracts where `term_end - unixepoch() <= renewal_alert_days * 86400` and `deleted_at IS NULL`.
- These are passed to a new Tauri command `get_renewal_alerts` and surfaced in the Today view's "what matters" section as amber/red pills based on days remaining (≤ 30 days amber, ≤ 7 days red).

---

## 4. CSV Import

### 4.1 Supported presets

| Preset | Date col | Amount col | Description col | Notes |
|---|---|---|---|---|
| Monzo | `Date` | `Amount` | `Name` + `Description` | Amount is signed (negative = debit) |
| Starling | `Date` | `Amount (GBP)` | `Counter Party` | Amount signed |
| Barclays | `Date` | `Amount` | `Memo` | Amount signed |
| HSBC | `Date` | `Debit Amount` + `Credit Amount` | `Transaction Description` | Two separate columns |
| Natwest | `Date` | `Value` | `Transaction type` + `Description` | Amount signed |
| Generic | user picks column index for date / amount / description | — | — |

### 4.2 Import flow

1. User clicks **Import CSV** button in Ledger header.
2. Drawer opens: bank preset dropdown + file picker.
3. On file select: parse first 5 rows, show preview table with auto-detected category matches (keyword matching against existing category names/defaults).
4. User confirms or cancels.
5. On confirm: insert all rows with `source = 'csv_import'`. Duplicate check: skip any row where a transaction already exists with the same `date`, `amount_pence`, and `description` (case-insensitive).

### 4.3 Auto-categorisation

Simple keyword matching on the description string (uppercase), checked in order:

| Keyword | Category |
|---|---|
| TESCO, SAINSBURY, WAITROSE, ALDI, LIDL, ASDA, MORRISONS | Groceries |
| UBER EATS, DELIVEROO, JUST EAT, MCDONALD, KFC, NANDO | Eating Out |
| TFL, UBER, NATIONAL RAIL, TRAINLINE | Transport |
| NETFLIX, SPOTIFY, AMAZON PRIME, DISNEY, APPLE | Subscriptions |
| BOOTS, PHARMACY, NHS, DENTIST | Health |
| O2, EE, VODAFONE, THREE, SKY, BT, VIRGIN | Subscriptions |
| PAYROLL, SALARY, WAGES | Income |

Unmatched → Uncategorised (no category_id). User can reassign after import via existing transaction edit flow.

---

## 5. Month-in-Review Panel

### 5.1 Persistent summary

Always visible at the top of the Ledger view. Shows for the selected month:

- **Total in** (sum of positive transactions) — green
- **Total out** (sum of negative transactions, shown as positive number) — red
- **Net** (in − |out|) — green if positive, red if negative
- By-category breakdown (existing `MonthlySummary` data, already computed)

### 5.2 AI narrative

Below the summary numbers: a *"Review with AI"* button.

On click:
1. Fetch `MonthlySummary` for the selected month.
2. Build a compact prompt: total in/out, each category spend vs budget, any contracts with upcoming renewals.
3. Send to Ollama (`qwen2.5:7b-instruct`). System prompt instructs it to write 2–3 sentences of plain English — what happened, what's notable, no financial advice, no bullet points.
4. Stream the response into the panel, replacing the button.
5. On completion: show a *"Refreshed just now"* timestamp + *"Refresh"* link.

This is **not a proposal** — it is read-only narrative displayed in-panel. No diff, no approve/reject flow.

### 5.3 Prompt template

```
You are a calm personal finance assistant. The user's spending for {month} {year}:

Total in: £{total_in}
Total out: £{total_out}
Net: £{net}

By category:
{for each category: "  - {emoji} {name}: £{spent} spent{, budget £{budget}, {over/under} by £{diff}}"}

{if renewals: "Upcoming contract renewals: {list}"}

Write 2-3 sentences summarising what happened this month in plain English. 
Be specific about notable categories. Do not give financial advice. No bullet points.
```

---

## 6. File Map

| Path | Status | Responsibility |
|---|---|---|
| `crates/core/migrations/V7__ledger_completions.sql` | Create | Schema additions |
| `crates/core/src/ledger/recurring.rs` | Create | `recurring_payment` DAL + `auto_insert_due` |
| `crates/core/src/ledger/contract.rs` | Create | `contract` DAL + `check_renewals` |
| `crates/core/src/ledger/mod.rs` | Modify | Add `pub mod recurring`, `pub mod contract` |
| `crates/core/src/ledger/transaction.rs` | Modify | Add `recurring_payment_id` field |
| `crates/app/src/ledger/csv_import.rs` | Create | CSV parsing, preset definitions, duplicate check |
| `crates/app/src/ledger/mod.rs` | Modify | Add `pub mod csv_import` |
| `crates/app/src/ledger/commands.rs` | Modify | Add 8 new Tauri commands |
| `crates/app/src/lib.rs` | Modify | Register new commands + auto-insert/renewal on setup |
| `apps/desktop/src/lib/ledger/ipc.ts` | Modify | Add new IPC functions + types |
| `apps/desktop/src/lib/ledger/state.ts` | Modify | Add recurring/contract/renewal state |
| `apps/desktop/src/components/Ledger/LedgerView.tsx` | Modify | Add new sections, import button, review panel |
| `apps/desktop/src/components/Ledger/RecurringSection.tsx` | Create | Collapsible list + add/edit/pause/delete |
| `apps/desktop/src/components/Ledger/ContractsSection.tsx` | Create | Collapsible list + countdown pills |
| `apps/desktop/src/components/Ledger/AddRecurringDrawer.tsx` | Create | Add/edit recurring payment form |
| `apps/desktop/src/components/Ledger/AddContractDrawer.tsx` | Create | Add/edit contract form |
| `apps/desktop/src/components/Ledger/CsvImportDrawer.tsx` | Create | Preset picker + file + preview + confirm |
| `apps/desktop/src/components/Ledger/MonthReviewPanel.tsx` | Create | Persistent summary + AI narrative |

---

## 7. New Tauri Commands

| Command | Type | Description |
|---|---|---|
| `ledger_list_recurring` | sync | All active recurring payments |
| `ledger_add_recurring` | sync | Insert new recurring payment |
| `ledger_update_recurring` | sync | Edit recurring payment |
| `ledger_delete_recurring` | sync | Soft-delete recurring payment |
| `ledger_list_contracts` | sync | All active contracts |
| `ledger_add_contract` | sync | Insert new contract |
| `ledger_update_contract` | sync | Edit contract |
| `ledger_delete_contract` | sync | Soft-delete contract |
| `ledger_get_renewal_alerts` | sync | Contracts due for renewal |
| `ledger_import_csv` | async | Parse + insert CSV rows, return count inserted + skipped |
| `ledger_ai_month_review` | async (streaming) | Stream Ollama narrative for selected month |

---

## 8. Error Handling

- **CSV parse error**: surface per-row errors in the preview drawer; skip bad rows rather than aborting the whole import.
- **CSV duplicate skip**: silently skip; final confirmation screen shows "X imported, Y skipped (duplicates)".
- **Ollama unavailable for month review**: show inline *"AI unavailable — start Ollama to use this feature"* with a dismiss button. Never block the persistent summary numbers.
- **Auto-insert failure**: log to `tracing::warn!`, don't crash app open. The insertion will be retried next app open.
