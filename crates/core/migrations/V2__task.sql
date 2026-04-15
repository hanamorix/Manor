CREATE TABLE task (
  id              INTEGER PRIMARY KEY,
  title           TEXT    NOT NULL,
  due_date        TEXT    NULL,
  completed_at    INTEGER NULL,
  created_at      INTEGER NOT NULL,
  proposal_id     INTEGER NULL REFERENCES proposal(id)
);

CREATE INDEX idx_task_open_due ON task (completed_at, due_date);
