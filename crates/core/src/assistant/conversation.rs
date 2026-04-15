//! Conversation threads. Phase 2 only ever has id=1 (rolling thread).

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conversation {
    pub id: i64,
    pub created_at: i64,
    pub title: String,
}

impl Conversation {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            created_at: row.get("created_at")?,
            title: row.get("title")?,
        })
    }
}

/// Return the default conversation, creating it (id=1) on first call.
pub fn get_or_create_default(conn: &Connection) -> Result<Conversation> {
    if let Some(c) = conn
        .query_row(
            "SELECT id, created_at, title FROM conversation WHERE id = 1",
            [],
            Conversation::from_row,
        )
        .ok()
    {
        return Ok(c);
    }

    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO conversation (id, created_at, title) VALUES (1, ?1, 'Manor')",
        [now],
    )?;

    Ok(Conversation {
        id: 1,
        created_at: now,
        title: "Manor".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    #[test]
    fn first_call_creates_default_conversation() {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();

        let c = get_or_create_default(&conn).unwrap();
        assert_eq!(c.id, 1);
        assert_eq!(c.title, "Manor");
        assert!(c.created_at > 0);
    }

    #[test]
    fn second_call_returns_same_conversation() {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();

        let a = get_or_create_default(&conn).unwrap();
        let b = get_or_create_default(&conn).unwrap();
        assert_eq!(a, b);
    }
}
