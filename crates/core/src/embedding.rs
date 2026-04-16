//! Embedding DAL — stores f32 vectors as BLOB; searches via in-Rust cosine.
//!
//! Design note: `sqlite-vec` extension is faster but introduces per-arch dylib
//! vendoring that complicates CI + code signing. For v0.1 household scale
//! (thousands of rows) a linear scan is fast enough. DAL surface is stable so
//! the storage can swap to `sqlite-vec` without callers noticing.

use anyhow::Result;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub model: String,
    pub dimension: i64,
    pub vector: Vec<f32>,
    pub entity_updated_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchHit {
    pub entity_type: String,
    pub entity_id: i64,
    pub score: f32, // cosine similarity in [-1, 1]; higher = closer
}

/// Row a background job needs to embed. Covers the entity sources we index.
#[derive(Debug, Clone)]
pub struct StaleRow {
    pub entity_type: String,
    pub entity_id: i64,
    pub updated_at: i64,
    pub text: String,
}

fn vector_to_bytes(v: &[f32]) -> Vec<u8> {
    bytemuck::cast_slice::<f32, u8>(v).to_vec()
}

fn vector_from_bytes(b: &[u8]) -> Vec<f32> {
    if b.len() % 4 != 0 {
        return Vec::new();
    }
    bytemuck::cast_slice::<u8, f32>(b).to_vec()
}

fn embedding_from_row(row: &Row) -> rusqlite::Result<Embedding> {
    let bytes: Vec<u8> = row.get("vector")?;
    Ok(Embedding {
        id: row.get("id")?,
        entity_type: row.get("entity_type")?,
        entity_id: row.get("entity_id")?,
        model: row.get("model")?,
        dimension: row.get("dimension")?,
        vector: vector_from_bytes(&bytes),
        entity_updated_at: row.get("entity_updated_at")?,
        created_at: row.get("created_at")?,
    })
}

/// Insert or replace the embedding for an entity/model pair.
pub fn upsert(
    conn: &Connection,
    entity_type: &str,
    entity_id: i64,
    model: &str,
    vector: &[f32],
    entity_updated_at: i64,
) -> Result<()> {
    let bytes = vector_to_bytes(vector);
    let dim = vector.len() as i64;
    conn.execute(
        "INSERT INTO embedding
         (entity_type, entity_id, model, dimension, vector, entity_updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(entity_type, entity_id, model) DO UPDATE
           SET dimension = excluded.dimension,
               vector = excluded.vector,
               entity_updated_at = excluded.entity_updated_at",
        params![entity_type, entity_id, model, dim, bytes, entity_updated_at],
    )?;
    Ok(())
}

/// Get a single embedding (most recent for the entity/model pair). Returns None if absent.
pub fn get(
    conn: &Connection,
    entity_type: &str,
    entity_id: i64,
    model: &str,
) -> Result<Option<Embedding>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, model, dimension, vector,
                entity_updated_at, created_at
         FROM embedding
         WHERE entity_type = ?1 AND entity_id = ?2 AND model = ?3",
    )?;
    let mut rows = stmt.query(params![entity_type, entity_id, model])?;
    match rows.next()? {
        Some(r) => Ok(Some(embedding_from_row(r)?)),
        None => Ok(None),
    }
}

/// Linear-scan cosine search. Filters by entity_types if non-empty.
/// Returns at most `limit` hits, sorted by descending similarity.
pub fn search_similar(
    conn: &Connection,
    query_vector: &[f32],
    model: &str,
    entity_types: &[&str],
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let mut stmt = conn.prepare(
        "SELECT entity_type, entity_id, vector
         FROM embedding WHERE model = ?1",
    )?;
    let rows_iter = stmt.query_map(params![model], |row| {
        let entity_type: String = row.get(0)?;
        let entity_id: i64 = row.get(1)?;
        let bytes: Vec<u8> = row.get(2)?;
        Ok((entity_type, entity_id, bytes))
    })?;

    let q_norm = norm(query_vector);
    if q_norm == 0.0 {
        return Ok(Vec::new());
    }
    let filter_set: std::collections::HashSet<&str> = entity_types.iter().copied().collect();

    let mut hits: Vec<SearchHit> = Vec::new();
    for row in rows_iter {
        let (entity_type, entity_id, bytes) = row?;
        if !filter_set.is_empty() && !filter_set.contains(entity_type.as_str()) {
            continue;
        }
        let v = vector_from_bytes(&bytes);
        if v.len() != query_vector.len() {
            continue; // dimension mismatch → skip
        }
        let d_norm = norm(&v);
        if d_norm == 0.0 {
            continue;
        }
        let dot: f32 = query_vector.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        let score = dot / (q_norm * d_norm);
        hits.push(SearchHit {
            entity_type,
            entity_id,
            score,
        });
    }

    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit);
    Ok(hits)
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Count embeddings stored, grouped by model. Used by the Settings/AI tab.
pub fn count_by_model(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt =
        conn.prepare("SELECT model, COUNT(*) FROM embedding GROUP BY model ORDER BY model")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Find rows across known entity types that need (re-)embedding.
/// A row is stale if:
///   - it has no embedding for `model`, OR
///   - its embedding.entity_updated_at < the row's updated_at
pub fn list_stale(conn: &Connection, model: &str, cap: usize) -> Result<Vec<StaleRow>> {
    let mut out: Vec<StaleRow> = Vec::new();

    // note: use body_md + updated_at
    let mut stmt = conn.prepare(
        "SELECT n.id, n.body_md, n.updated_at
         FROM note n
         LEFT JOIN embedding e ON e.entity_type = 'note' AND e.entity_id = n.id AND e.model = ?1
         WHERE n.deleted_at IS NULL
           AND (e.id IS NULL OR e.entity_updated_at < n.updated_at)
         LIMIT ?2",
    )?;
    for row in stmt.query_map(params![model, cap as i64], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })? {
        let (id, text, updated_at) = row?;
        out.push(StaleRow {
            entity_type: "note".into(),
            entity_id: id,
            updated_at,
            text,
        });
        if out.len() >= cap {
            return Ok(out);
        }
    }

    // task: title; task table doesn't have updated_at, so use created_at as proxy
    let remaining = cap.saturating_sub(out.len());
    if remaining > 0 {
        let mut stmt = conn.prepare(
            "SELECT t.id, t.title, t.created_at
             FROM task t
             LEFT JOIN embedding e ON e.entity_type = 'task' AND e.entity_id = t.id AND e.model = ?1
             WHERE e.id IS NULL
             LIMIT ?2",
        )?;
        for row in stmt.query_map(params![model, remaining as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })? {
            let (id, text, updated_at) = row?;
            out.push(StaleRow {
                entity_type: "task".into(),
                entity_id: id,
                updated_at,
                text,
            });
            if out.len() >= cap {
                return Ok(out);
            }
        }
    }

    // ledger_transaction: description; use created_at as proxy for updated_at
    let remaining = cap.saturating_sub(out.len());
    if remaining > 0 {
        let mut stmt = conn.prepare(
            "SELECT lt.id, lt.description, lt.created_at
             FROM ledger_transaction lt
             LEFT JOIN embedding e ON e.entity_type = 'ledger_transaction'
                                   AND e.entity_id = lt.id AND e.model = ?1
             WHERE lt.deleted_at IS NULL
               AND e.id IS NULL
             LIMIT ?2",
        )?;
        for row in stmt.query_map(params![model, remaining as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })? {
            let (id, text, updated_at) = row?;
            out.push(StaleRow {
                entity_type: "ledger_transaction".into(),
                entity_id: id,
                updated_at,
                text,
            });
            if out.len() >= cap {
                return Ok(out);
            }
        }
    }

    Ok(out)
}

/// Delete all embeddings (used by "Rebuild" button in Settings).
pub fn clear_all(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM embedding", [])?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn upsert_stores_vector_and_get_reads_it_back() {
        let (_d, conn) = fresh_conn();
        let v = vec![0.1, 0.2, 0.3, 0.4];
        upsert(&conn, "note", 42, "nomic", &v, 1000).unwrap();
        let got = get(&conn, "note", 42, "nomic").unwrap().unwrap();
        assert_eq!(got.vector, v);
        assert_eq!(got.dimension, 4);
    }

    #[test]
    fn upsert_replaces_existing_entity_model_pair() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, "note", 42, "nomic", &[1.0, 0.0], 1000).unwrap();
        upsert(&conn, "note", 42, "nomic", &[0.0, 1.0], 2000).unwrap();
        let got = get(&conn, "note", 42, "nomic").unwrap().unwrap();
        assert_eq!(got.vector, vec![0.0, 1.0]);
        assert_eq!(got.entity_updated_at, 2000);
        // Only one row.
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM embedding", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn search_returns_hits_ordered_by_cosine() {
        let (_d, conn) = fresh_conn();
        // 3 vectors; query nearest to #1.
        upsert(&conn, "note", 1, "nomic", &[1.0, 0.0, 0.0], 1000).unwrap();
        upsert(&conn, "note", 2, "nomic", &[0.0, 1.0, 0.0], 1000).unwrap();
        upsert(&conn, "note", 3, "nomic", &[0.9, 0.1, 0.0], 1000).unwrap();

        let hits = search_similar(&conn, &[1.0, 0.0, 0.0], "nomic", &[], 2).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].entity_id, 1);
        assert!(hits[0].score > 0.99);
        assert_eq!(hits[1].entity_id, 3);
    }

    #[test]
    fn search_filters_by_entity_type() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, "note", 1, "nomic", &[1.0, 0.0], 1000).unwrap();
        upsert(&conn, "task", 2, "nomic", &[1.0, 0.0], 1000).unwrap();
        let hits = search_similar(&conn, &[1.0, 0.0], "nomic", &["note"], 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_type, "note");
    }

    #[test]
    fn search_skips_dimension_mismatches() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, "note", 1, "nomic", &[1.0, 0.0], 1000).unwrap();
        upsert(&conn, "note", 2, "nomic", &[1.0, 0.0, 0.0], 1000).unwrap();
        let hits = search_similar(&conn, &[1.0, 0.0], "nomic", &[], 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_id, 1);
    }

    #[test]
    fn list_stale_finds_unembedded_note() {
        let (_d, conn) = fresh_conn();
        let n = crate::note::insert(&conn, "Hello world", None, None).unwrap();
        let stale = list_stale(&conn, "nomic", 10).unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].entity_type, "note");
        assert_eq!(stale[0].entity_id, n.id);
        assert_eq!(stale[0].text, "Hello world");
    }

    #[test]
    fn list_stale_skips_already_embedded() {
        let (_d, conn) = fresh_conn();
        let n = crate::note::insert(&conn, "Hello", None, None).unwrap();
        upsert(&conn, "note", n.id, "nomic", &[0.1, 0.2], n.updated_at).unwrap();
        let stale = list_stale(&conn, "nomic", 10).unwrap();
        assert!(stale.iter().find(|s| s.entity_id == n.id).is_none());
    }

    #[test]
    fn list_stale_picks_up_updated_notes() {
        let (_d, conn) = fresh_conn();
        let n = crate::note::insert(&conn, "original", None, None).unwrap();
        upsert(&conn, "note", n.id, "nomic", &[0.1, 0.2], n.updated_at).unwrap();
        // Simulate an edit: bump updated_at on the note but don't re-embed.
        conn.execute(
            "UPDATE note SET updated_at = updated_at + 10 WHERE id = ?1",
            [n.id],
        )
        .unwrap();
        let stale = list_stale(&conn, "nomic", 10).unwrap();
        assert_eq!(stale.iter().filter(|s| s.entity_id == n.id).count(), 1);
    }

    #[test]
    fn count_by_model_groups_correctly() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, "note", 1, "nomic", &[1.0, 0.0], 1000).unwrap();
        upsert(&conn, "note", 2, "nomic", &[0.0, 1.0], 1000).unwrap();
        upsert(&conn, "note", 3, "gpt", &[1.0, 0.0, 0.0], 1000).unwrap();
        let counts = count_by_model(&conn).unwrap();
        assert_eq!(
            counts,
            vec![("gpt".to_string(), 1), ("nomic".to_string(), 2)]
        );
    }

    #[test]
    fn clear_all_empties_the_table() {
        let (_d, conn) = fresh_conn();
        upsert(&conn, "note", 1, "nomic", &[1.0, 0.0], 1000).unwrap();
        upsert(&conn, "note", 2, "nomic", &[0.0, 1.0], 1000).unwrap();
        let n = clear_all(&conn).unwrap();
        assert_eq!(n, 2);
        assert_eq!(count_by_model(&conn).unwrap().len(), 0);
    }
}
