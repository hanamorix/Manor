CREATE TABLE conversation (
  id         INTEGER PRIMARY KEY,
  created_at INTEGER NOT NULL,
  title      TEXT    NOT NULL DEFAULT 'Manor'
);

CREATE TABLE proposal (
  id                  INTEGER PRIMARY KEY,
  kind                TEXT    NOT NULL,
  rationale           TEXT    NOT NULL,
  diff                TEXT    NOT NULL,
  status              TEXT    NOT NULL DEFAULT 'pending',
  proposed_at         INTEGER NOT NULL,
  applied_at          INTEGER NULL,
  skill               TEXT    NOT NULL,
  remote_call_log_id  INTEGER NULL
);

CREATE TABLE message (
  id              INTEGER PRIMARY KEY,
  conversation_id INTEGER NOT NULL REFERENCES conversation(id),
  role            TEXT    NOT NULL,
  content         TEXT    NOT NULL,
  created_at      INTEGER NOT NULL,
  seen            INTEGER NOT NULL DEFAULT 0,
  proposal_id     INTEGER NULL REFERENCES proposal(id)
);

CREATE INDEX idx_message_conversation_created ON message (conversation_id, created_at);
