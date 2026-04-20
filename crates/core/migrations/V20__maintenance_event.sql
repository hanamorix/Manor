-- V20__maintenance_event.sql
-- L4c: maintenance event log + Ledger transaction linkage.

CREATE TABLE maintenance_event (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    schedule_id     TEXT REFERENCES maintenance_schedule(id),
    title           TEXT NOT NULL DEFAULT '',
    completed_date  TEXT NOT NULL,
    cost_pence      INTEGER,
    currency        TEXT NOT NULL DEFAULT 'GBP',
    notes           TEXT NOT NULL DEFAULT '',
    transaction_id  INTEGER REFERENCES ledger_transaction(id),
    source          TEXT NOT NULL DEFAULT 'manual'
                    CHECK (source IN ('manual','backfill')),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_evt_asset     ON maintenance_event(asset_id);
CREATE INDEX idx_evt_schedule  ON maintenance_event(schedule_id) WHERE schedule_id IS NOT NULL;
CREATE INDEX idx_evt_completed ON maintenance_event(completed_date) WHERE deleted_at IS NULL;
CREATE INDEX idx_evt_deleted   ON maintenance_event(deleted_at);

CREATE UNIQUE INDEX idx_evt_tx_unique
    ON maintenance_event(transaction_id)
    WHERE transaction_id IS NOT NULL AND deleted_at IS NULL;

INSERT INTO maintenance_event (
    id, asset_id, schedule_id, title, completed_date,
    cost_pence, currency, notes, transaction_id, source,
    created_at, updated_at, deleted_at
)
SELECT
    lower(hex(randomblob(16))),
    ms.asset_id,
    ms.id,
    ms.task,
    ms.last_done_date,
    NULL, 'GBP', '', NULL, 'backfill',
    unixepoch(), unixepoch(), NULL
FROM maintenance_schedule ms
WHERE ms.last_done_date IS NOT NULL
  AND ms.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM maintenance_event me
      WHERE me.schedule_id = ms.id AND me.source = 'backfill'
  );
