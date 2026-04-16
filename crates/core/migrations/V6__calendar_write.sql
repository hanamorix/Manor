-- V6__calendar_write.sql
-- Extend event table with write-support columns
ALTER TABLE event ADD COLUMN event_url               TEXT;
ALTER TABLE event ADD COLUMN etag                    TEXT;
ALTER TABLE event ADD COLUMN description             TEXT;
ALTER TABLE event ADD COLUMN location                TEXT;
ALTER TABLE event ADD COLUMN all_day                 INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event ADD COLUMN is_recurring_occurrence INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event ADD COLUMN parent_event_url        TEXT;
ALTER TABLE event ADD COLUMN occurrence_dtstart      TEXT;
ALTER TABLE event ADD COLUMN deleted_at              INTEGER;

-- Calendar account default calendar
ALTER TABLE calendar_account ADD COLUMN default_calendar_url TEXT;

-- Persisted calendar list (one row per calendar URL per account)
CREATE TABLE calendar (
  id                  INTEGER PRIMARY KEY,
  calendar_account_id INTEGER NOT NULL REFERENCES calendar_account(id) ON DELETE CASCADE,
  url                 TEXT    NOT NULL,
  display_name        TEXT,
  created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
  UNIQUE(calendar_account_id, url)
);

CREATE INDEX idx_calendar_account_id ON calendar(calendar_account_id);
