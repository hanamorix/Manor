-- V23__drop_bank_sync.sql
-- Retire the GoCardless bank-sync surface. After this migration:
--   * bank_account and gocardless_institution_cache tables are gone
--   * ledger_transaction loses its bank_account_id + external_id columns and
--     the UNIQUE(bank_account_id, external_id) constraint and
--     idx_ledger_transaction_bank_account index that reference them
--   * Historical rows with source='sync' are relabelled to 'csv_import_legacy'
--     so the data survives but the dead source value disappears.
--
-- ledger_transaction is rebuilt via the SQLite table-swap pattern because
-- simple ALTER TABLE DROP COLUMN can't drop a column that participates in a
-- table-level UNIQUE constraint.

-- Relabel historical sync rows in place before the rebuild.
UPDATE ledger_transaction
    SET source = 'csv_import_legacy'
    WHERE source = 'sync';

-- Rebuild ledger_transaction without bank_account_id, external_id, or the
-- associated UNIQUE constraint. Column order preserved so downstream code
-- that queries by name is unaffected.
CREATE TABLE ledger_transaction__new (
    id                    INTEGER PRIMARY KEY,
    amount_pence          INTEGER NOT NULL,
    currency              TEXT    NOT NULL DEFAULT 'GBP',
    description           TEXT    NOT NULL,
    merchant              TEXT,
    category_id           INTEGER REFERENCES category(id),
    date                  INTEGER NOT NULL,
    source                TEXT    NOT NULL DEFAULT 'manual',
    note                  TEXT,
    created_at            INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at            INTEGER,
    recurring_payment_id  INTEGER
);

INSERT INTO ledger_transaction__new
    (id, amount_pence, currency, description, merchant, category_id, date,
     source, note, created_at, deleted_at, recurring_payment_id)
SELECT
    id, amount_pence, currency, description, merchant, category_id, date,
    source, note, created_at, deleted_at, recurring_payment_id
FROM ledger_transaction;

DROP INDEX IF EXISTS idx_ledger_transaction_date;
DROP INDEX IF EXISTS idx_ledger_transaction_category;
DROP INDEX IF EXISTS idx_ledger_transaction_bank_account;

DROP TABLE ledger_transaction;
ALTER TABLE ledger_transaction__new RENAME TO ledger_transaction;

CREATE INDEX idx_ledger_transaction_date     ON ledger_transaction(date);
CREATE INDEX idx_ledger_transaction_category ON ledger_transaction(category_id);

DROP TABLE IF EXISTS gocardless_institution_cache;
DROP TABLE IF EXISTS bank_account;
