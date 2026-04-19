-- V19__maintenance_schedule.sql
-- L4b Maintenance Schedules: per-asset time-based maintenance.

CREATE TABLE maintenance_schedule (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    task            TEXT NOT NULL,
    interval_months INTEGER NOT NULL CHECK (interval_months >= 1),
    last_done_date  TEXT,
    next_due_date   TEXT NOT NULL,
    notes           TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_maint_asset    ON maintenance_schedule(asset_id);
CREATE INDEX idx_maint_deleted  ON maintenance_schedule(deleted_at);
CREATE INDEX idx_maint_next_due ON maintenance_schedule(next_due_date) WHERE deleted_at IS NULL;
