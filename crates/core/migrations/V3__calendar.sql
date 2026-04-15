CREATE TABLE calendar_account (
  id                INTEGER PRIMARY KEY,
  display_name      TEXT    NOT NULL,
  server_url        TEXT    NOT NULL,
  username          TEXT    NOT NULL,
  last_synced_at    INTEGER NULL,
  last_error        TEXT    NULL,
  created_at        INTEGER NOT NULL,
  UNIQUE (server_url, username)
);

CREATE TABLE event (
  id                  INTEGER PRIMARY KEY,
  calendar_account_id INTEGER NOT NULL REFERENCES calendar_account(id) ON DELETE CASCADE,
  external_id         TEXT    NOT NULL,
  title               TEXT    NOT NULL,
  start_at            INTEGER NOT NULL,
  end_at              INTEGER NOT NULL,
  created_at          INTEGER NOT NULL,
  UNIQUE (calendar_account_id, external_id)
);

CREATE INDEX idx_event_start_at ON event (start_at);
