-- V10__embeddings.sql
-- Local semantic search. Vectors stored as BLOB; cosine search runs in Rust.
-- Swap to sqlite-vec later if row count demands ANN speed.

CREATE TABLE embedding (
    id          INTEGER PRIMARY KEY,
    entity_type TEXT    NOT NULL,
    entity_id   INTEGER NOT NULL,
    model       TEXT    NOT NULL,
    dimension   INTEGER NOT NULL,
    vector      BLOB    NOT NULL,       -- raw f32 little-endian bytes, dimension * 4 bytes
    entity_updated_at INTEGER NOT NULL, -- snapshot of the source row's updated_at at embed time
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(entity_type, entity_id, model)
);

CREATE INDEX idx_embedding_lookup ON embedding(entity_type, entity_id);
