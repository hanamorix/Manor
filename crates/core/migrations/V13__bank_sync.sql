-- V13__bank_sync.sql
-- Phase 5d: GoCardless Bank Account Data integration.

-- Extend the Phase 5a bank_account stub with GoCardless-specific fields.
ALTER TABLE bank_account ADD COLUMN institution_id              TEXT;
ALTER TABLE bank_account ADD COLUMN institution_logo_url        TEXT;
ALTER TABLE bank_account ADD COLUMN reference                   TEXT;
ALTER TABLE bank_account ADD COLUMN requisition_created_at      INTEGER;
ALTER TABLE bank_account ADD COLUMN max_historical_days_granted INTEGER;
ALTER TABLE bank_account ADD COLUMN sync_paused_reason          TEXT;
ALTER TABLE bank_account ADD COLUMN initial_sync_completed_at   INTEGER;

-- Rename for accuracy — the lifetime is requisition-bound, not token-bound.
ALTER TABLE bank_account RENAME COLUMN token_expires_at TO requisition_expires_at;

-- Per-country 24h cache of /institutions responses.
CREATE TABLE gocardless_institution_cache (
    country                TEXT    NOT NULL,
    institution_id         TEXT    NOT NULL,
    name                   TEXT    NOT NULL,
    bic                    TEXT,
    logo_url               TEXT,
    max_historical_days    INTEGER NOT NULL,
    access_valid_for_days  INTEGER NOT NULL,
    fetched_at             INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (country, institution_id)
);

CREATE INDEX idx_gocardless_institution_cache_fetched
    ON gocardless_institution_cache(fetched_at);

-- Dev-only sandbox toggle.
INSERT OR IGNORE INTO setting (key, value) VALUES ('bank_sandbox_enabled', 'false');
