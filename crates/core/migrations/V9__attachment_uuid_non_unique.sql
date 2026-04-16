-- V8 created attachment.uuid as UNIQUE, but the DAL dedup design allows multiple
-- rows to share the same uuid (same bytes → same file, different metadata rows).
-- SQLite cannot DROP a constraint in-place; recreate the table without it.

PRAGMA foreign_keys = OFF;

CREATE TABLE attachment_new (
    id            INTEGER PRIMARY KEY,
    uuid          TEXT    NOT NULL,
    original_name TEXT    NOT NULL,
    mime_type     TEXT    NOT NULL,
    size_bytes    INTEGER NOT NULL,
    sha256        TEXT    NOT NULL,
    entity_type   TEXT,
    entity_id     INTEGER,
    created_at    INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at    INTEGER
);

INSERT INTO attachment_new SELECT * FROM attachment;
DROP TABLE attachment;
ALTER TABLE attachment_new RENAME TO attachment;

CREATE INDEX idx_attachment_entity ON attachment(entity_type, entity_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_attachment_sha    ON attachment(sha256);
CREATE INDEX idx_attachment_uuid   ON attachment(uuid);

PRAGMA foreign_keys = ON;
