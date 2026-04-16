-- V8__foundation_tables.sql
-- Foundation tables for v0.1 completion: setting, person upgrade, household,
-- tag + tag_link, note, attachment.

-- ── setting: non-secret key/value store ──────────────────────────────────────
CREATE TABLE setting (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- ── person: upgrade existing minimal table to full schema ────────────────────
-- Existing columns: id, name, created_at
ALTER TABLE person ADD COLUMN kind       TEXT NOT NULL DEFAULT 'member';
ALTER TABLE person ADD COLUMN email      TEXT;
ALTER TABLE person ADD COLUMN phone      TEXT;
ALTER TABLE person ADD COLUMN note       TEXT;
ALTER TABLE person ADD COLUMN updated_at INTEGER NOT NULL DEFAULT (unixepoch());
ALTER TABLE person ADD COLUMN deleted_at INTEGER;

-- Kinds enforced in DAL, not DB (SQLite ALTER can't add CHECK).
-- Allowed values: 'owner' | 'member' | 'contact' | 'provider' | 'vendor'.

-- ── household: singleton config ──────────────────────────────────────────────
CREATE TABLE household (
    id                 INTEGER PRIMARY KEY CHECK (id = 1),
    owner_person_id    INTEGER REFERENCES person(id),
    working_hours_json TEXT NOT NULL DEFAULT '{"mon":[9,17],"tue":[9,17],"wed":[9,17],"thu":[9,17],"fri":[9,17],"sat":[],"sun":[]}',
    dnd_windows_json   TEXT NOT NULL DEFAULT '[]',
    created_at         INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at         INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Seed singleton row. Owner_person_id stays NULL until user picks/creates one.
INSERT INTO household (id) VALUES (1);

-- ── tag + tag_link ───────────────────────────────────────────────────────────
CREATE TABLE tag (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color      TEXT NOT NULL DEFAULT '#888',
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE tag_link (
    id          INTEGER PRIMARY KEY,
    tag_id      INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    entity_type TEXT    NOT NULL,
    entity_id   INTEGER NOT NULL,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(tag_id, entity_type, entity_id)
);

CREATE INDEX idx_tag_link_entity ON tag_link(entity_type, entity_id);

-- ── note ─────────────────────────────────────────────────────────────────────
CREATE TABLE note (
    id          INTEGER PRIMARY KEY,
    body_md     TEXT NOT NULL,
    entity_type TEXT,
    entity_id   INTEGER,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at  INTEGER
);

CREATE INDEX idx_note_entity ON note(entity_type, entity_id) WHERE deleted_at IS NULL;

-- ── attachment ───────────────────────────────────────────────────────────────
CREATE TABLE attachment (
    id            INTEGER PRIMARY KEY,
    uuid          TEXT    NOT NULL UNIQUE,
    original_name TEXT    NOT NULL,
    mime_type     TEXT    NOT NULL,
    size_bytes    INTEGER NOT NULL,
    sha256        TEXT    NOT NULL,
    entity_type   TEXT,
    entity_id     INTEGER,
    created_at    INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at    INTEGER
);

CREATE INDEX idx_attachment_entity ON attachment(entity_type, entity_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_attachment_sha    ON attachment(sha256);
