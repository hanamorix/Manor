//! Messages in the rolling conversation.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            other => anyhow::bail!("unknown role: {other}"),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: i64,
    pub conversation_id: i64,
    pub role: Role,
    pub content: String,
    pub created_at: i64,
    pub seen: bool,
    pub proposal_id: Option<i64>,
}

impl Message {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let role_str: String = row.get("role")?;
        Ok(Self {
            id: row.get("id")?,
            conversation_id: row.get("conversation_id")?,
            role: role_str.parse().map_err(|e: anyhow::Error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
                )
            })?,
            content: row.get("content")?,
            created_at: row.get("created_at")?,
            seen: row.get::<_, i64>("seen")? != 0,
            proposal_id: row.get("proposal_id")?,
        })
    }
}

/// Insert a new message. Returns the new row id.
/// User messages are always inserted with `seen=true`; assistant/system messages with `seen=false`.
pub fn insert(
    conn: &Connection,
    conversation_id: i64,
    role: Role,
    content: &str,
) -> Result<i64> {
    let now_ms = Utc::now().timestamp_millis();
    let seen = matches!(role, Role::User) as i64;
    conn.execute(
        "INSERT INTO message (conversation_id, role, content, created_at, seen)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![conversation_id, role.as_str(), content, now_ms, seen],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Append text to the content of an existing message (used while streaming assistant replies).
pub fn append_content(conn: &Connection, id: i64, fragment: &str) -> Result<()> {
    conn.execute(
        "UPDATE message SET content = content || ?1 WHERE id = ?2",
        params![fragment, id],
    )?;
    Ok(())
}

/// List the most recent `limit` messages for a conversation, oldest-first within the window,
/// starting `offset` messages back from newest.
pub fn list(
    conn: &Connection,
    conversation_id: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, created_at, seen, proposal_id
         FROM message
         WHERE conversation_id = ?1
         ORDER BY created_at DESC
         LIMIT ?2 OFFSET ?3",
    )?;
    let mut rows: Vec<Message> = stmt
        .query_map(params![conversation_id, limit, offset], Message::from_row)?
        .collect::<rusqlite::Result<_>>()?;
    rows.reverse(); // oldest-first within the window
    Ok(rows)
}

/// Mark a batch of messages as seen.
pub fn mark_seen(conn: &Connection, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
    let sql = format!("UPDATE message SET seen = 1 WHERE id IN ({placeholders})");
    let params_owned: Vec<&dyn rusqlite::ToSql> =
        ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
    conn.execute(&sql, params_owned.as_slice())?;
    Ok(())
}

/// Count assistant messages that have not been seen.
pub fn unread_count(conn: &Connection, conversation_id: i64) -> Result<u32> {
    let c: i64 = conn.query_row(
        "SELECT COUNT(*) FROM message
         WHERE conversation_id = ?1 AND role = 'assistant' AND seen = 0",
        [conversation_id],
        |r| r.get(0),
    )?;
    Ok(c as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::{conversation, db};
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let conv = conversation::get_or_create_default(&conn).unwrap();
        (dir, conn, conv.id)
    }

    #[test]
    fn insert_user_message_is_seen() {
        let (_d, conn, cid) = fresh_conn();
        let id = insert(&conn, cid, Role::User, "hello").unwrap();
        let msgs = list(&conn, cid, 10, 0).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].id, id);
        assert_eq!(msgs[0].content, "hello");
        assert!(msgs[0].seen);
        assert_eq!(msgs[0].role, Role::User);
    }

    #[test]
    fn insert_assistant_message_is_unseen() {
        let (_d, conn, cid) = fresh_conn();
        insert(&conn, cid, Role::Assistant, "pong").unwrap();
        assert_eq!(unread_count(&conn, cid).unwrap(), 1);
    }

    #[test]
    fn append_content_grows_the_message() {
        let (_d, conn, cid) = fresh_conn();
        let id = insert(&conn, cid, Role::Assistant, "").unwrap();
        append_content(&conn, id, "hel").unwrap();
        append_content(&conn, id, "lo").unwrap();
        let msgs = list(&conn, cid, 10, 0).unwrap();
        assert_eq!(msgs[0].content, "hello");
    }

    #[test]
    fn list_returns_oldest_first_within_window() {
        let (_d, conn, cid) = fresh_conn();
        insert(&conn, cid, Role::User, "a").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        insert(&conn, cid, Role::Assistant, "b").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        insert(&conn, cid, Role::User, "c").unwrap();

        let msgs = list(&conn, cid, 10, 0).unwrap();
        let contents: Vec<&str> = msgs.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, vec!["a", "b", "c"]);
    }

    #[test]
    fn mark_seen_clears_unread_count() {
        let (_d, conn, cid) = fresh_conn();
        let a = insert(&conn, cid, Role::Assistant, "x").unwrap();
        let b = insert(&conn, cid, Role::Assistant, "y").unwrap();
        assert_eq!(unread_count(&conn, cid).unwrap(), 2);
        mark_seen(&conn, &[a, b]).unwrap();
        assert_eq!(unread_count(&conn, cid).unwrap(), 0);
    }
}
