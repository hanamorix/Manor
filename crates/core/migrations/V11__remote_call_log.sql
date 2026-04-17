-- V11__remote_call_log.sql
-- Audit log for every remote LLM call. Soft-deletable so it integrates with Trash.
--
-- Privacy guarantee: prompt_redacted contains the bytes that left this Mac, not
-- the original input. Unredacted prompts are never persisted.

CREATE TABLE remote_call_log (
    id                  INTEGER PRIMARY KEY,
    provider            TEXT    NOT NULL,
    model               TEXT    NOT NULL,
    skill               TEXT    NOT NULL,
    user_visible_reason TEXT    NOT NULL,
    prompt_redacted     TEXT    NOT NULL,
    response_text       TEXT,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cost_pence          INTEGER,
    redaction_count     INTEGER NOT NULL DEFAULT 0,
    error               TEXT,
    started_at          INTEGER NOT NULL,
    completed_at        INTEGER,
    deleted_at          INTEGER
);

CREATE INDEX idx_remote_call_log_month ON remote_call_log(started_at)
  WHERE deleted_at IS NULL;
