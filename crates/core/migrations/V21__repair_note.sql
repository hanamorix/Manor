-- V21__repair_note.sql
-- L4d: right-to-repair search history.

CREATE TABLE repair_note (
    id              TEXT PRIMARY KEY,
    asset_id        TEXT NOT NULL REFERENCES asset(id),
    symptom         TEXT NOT NULL,
    body_md         TEXT NOT NULL,
    sources         TEXT NOT NULL,
    video_sources   TEXT,
    tier            TEXT NOT NULL
                    CHECK (tier IN ('ollama','claude')),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE INDEX idx_repair_asset    ON repair_note(asset_id);
CREATE INDEX idx_repair_created  ON repair_note(created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_repair_deleted  ON repair_note(deleted_at);
