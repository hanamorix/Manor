-- Category — fixed defaults + user-editable
CREATE TABLE category (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    emoji       TEXT    NOT NULL DEFAULT '💳',
    is_income   INTEGER NOT NULL DEFAULT 0,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_default  INTEGER NOT NULL DEFAULT 0,
    deleted_at  INTEGER
);

INSERT INTO category (id, name, emoji, is_income, sort_order, is_default) VALUES
    (1,  'Groceries',     '🛒', 0,  1, 1),
    (2,  'Eating Out',    '🍕', 0,  2, 1),
    (3,  'Transport',     '🚇', 0,  3, 1),
    (4,  'Utilities',     '⚡', 0,  4, 1),
    (5,  'Subscriptions', '📱', 0,  5, 1),
    (6,  'Health',        '💊', 0,  6, 1),
    (7,  'Shopping',      '🛍', 0,  7, 1),
    (8,  'Entertainment', '🎬', 0,  8, 1),
    (9,  'Other',         '💳', 0,  9, 1),
    (10, 'Income',        '💼', 1, 10, 1);

-- Bank account stub (Phase 5b will populate this)
CREATE TABLE bank_account (
    id               INTEGER PRIMARY KEY,
    provider         TEXT    NOT NULL,
    institution_name TEXT    NOT NULL,
    account_name     TEXT    NOT NULL,
    account_type     TEXT    NOT NULL DEFAULT 'current',
    currency         TEXT    NOT NULL DEFAULT 'GBP',
    external_id      TEXT    NOT NULL,
    requisition_id   TEXT,
    token_expires_at INTEGER,
    last_synced_at   INTEGER,
    last_nudge_at    INTEGER,
    created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at       INTEGER
);

-- Transaction — manual or bank-synced
CREATE TABLE ledger_transaction (
    id              INTEGER PRIMARY KEY,
    bank_account_id INTEGER REFERENCES bank_account(id),
    external_id     TEXT,
    amount_pence    INTEGER NOT NULL,
    currency        TEXT    NOT NULL DEFAULT 'GBP',
    description     TEXT    NOT NULL,
    merchant        TEXT,
    category_id     INTEGER REFERENCES category(id),
    date            INTEGER NOT NULL,
    source          TEXT    NOT NULL DEFAULT 'manual',
    note            TEXT,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at      INTEGER,
    UNIQUE(bank_account_id, external_id)
);

CREATE INDEX idx_ledger_transaction_date     ON ledger_transaction(date);
CREATE INDEX idx_ledger_transaction_category ON ledger_transaction(category_id);
CREATE INDEX idx_ledger_transaction_bank_account ON ledger_transaction(bank_account_id);

-- Monthly budget per category
CREATE TABLE budget (
    id           INTEGER PRIMARY KEY,
    category_id  INTEGER NOT NULL REFERENCES category(id),
    amount_pence INTEGER NOT NULL,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at   INTEGER,
    UNIQUE(category_id)
);
