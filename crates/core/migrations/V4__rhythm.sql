CREATE TABLE person (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE TABLE chore (
    id          INTEGER PRIMARY KEY,
    title       TEXT    NOT NULL,
    emoji       TEXT    NOT NULL DEFAULT '🧹',
    rrule       TEXT    NOT NULL,
    next_due    INTEGER NOT NULL,
    rotation    TEXT    NOT NULL DEFAULT 'none',
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);

CREATE INDEX idx_chore_next_due ON chore(next_due) WHERE deleted_at IS NULL AND active = 1;

CREATE TABLE chore_completion (
    id              INTEGER PRIMARY KEY,
    chore_id        INTEGER NOT NULL REFERENCES chore(id) ON DELETE CASCADE,
    completed_at    INTEGER NOT NULL,
    completed_by    INTEGER REFERENCES person(id),
    created_at      INTEGER NOT NULL
);

CREATE INDEX idx_chore_completion_chore ON chore_completion(chore_id);
CREATE INDEX idx_chore_completion_person ON chore_completion(completed_by) WHERE completed_by IS NOT NULL;

CREATE TABLE rotation (
    id          INTEGER PRIMARY KEY,
    chore_id    INTEGER NOT NULL REFERENCES chore(id) ON DELETE CASCADE,
    person_id   INTEGER NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    current     INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);

CREATE INDEX idx_rotation_chore ON rotation(chore_id);

CREATE TABLE time_block (
    id                          INTEGER PRIMARY KEY,
    title                       TEXT    NOT NULL,
    kind                        TEXT    NOT NULL,
    date                        INTEGER NOT NULL,
    start_time                  TEXT    NOT NULL,
    end_time                    TEXT    NOT NULL,
    rrule                       TEXT,
    is_pattern                  INTEGER NOT NULL DEFAULT 0,
    pattern_nudge_dismissed_at  INTEGER,
    created_at                  INTEGER NOT NULL,
    deleted_at                  INTEGER
);

CREATE INDEX idx_time_block_date ON time_block(date) WHERE deleted_at IS NULL;
