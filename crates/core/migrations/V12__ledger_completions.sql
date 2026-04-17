-- V12__ledger_completions.sql
-- Phase 5c: recurring payments + contracts + auto-insert traceability.

CREATE TABLE recurring_payment (
    id           INTEGER PRIMARY KEY,
    description  TEXT    NOT NULL,
    amount_pence INTEGER NOT NULL,
    currency     TEXT    NOT NULL DEFAULT 'GBP',
    category_id  INTEGER REFERENCES category(id),
    day_of_month INTEGER NOT NULL CHECK (day_of_month BETWEEN 1 AND 28),
    active       INTEGER NOT NULL DEFAULT 1,
    note         TEXT,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at   INTEGER
);

-- Trace which recurring template produced a transaction.
ALTER TABLE ledger_transaction ADD COLUMN recurring_payment_id INTEGER REFERENCES recurring_payment(id);

CREATE INDEX idx_ledger_transaction_recurring ON ledger_transaction(recurring_payment_id);

-- Contracts with renewal alerts.
CREATE TABLE contract (
    id                   INTEGER PRIMARY KEY,
    provider             TEXT    NOT NULL,
    kind                 TEXT    NOT NULL DEFAULT 'other',
    description          TEXT,
    monthly_cost_pence   INTEGER NOT NULL,
    term_start           INTEGER NOT NULL,
    term_end             INTEGER NOT NULL,
    exit_fee_pence       INTEGER,
    renewal_alert_days   INTEGER NOT NULL DEFAULT 30,
    recurring_payment_id INTEGER REFERENCES recurring_payment(id),
    note                 TEXT,
    created_at           INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at           INTEGER
);

CREATE INDEX idx_contract_term_end ON contract(term_end) WHERE deleted_at IS NULL;
