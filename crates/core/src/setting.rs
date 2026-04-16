//! Key-value setting store. Non-secret app preferences only.
//! Secrets (API keys, passphrases) belong in macOS Keychain.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let result: Option<String> = conn
        .query_row(
            "SELECT value FROM setting WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .ok();
    Ok(result)
}

pub fn get_or_default(conn: &Connection, key: &str, default: &str) -> Result<String> {
    Ok(get(conn, key)?.unwrap_or_else(|| default.to_string()))
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO setting (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                        updated_at = unixepoch()",
        params![key, value],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM setting WHERE key = ?1", [key])?;
    Ok(())
}

pub fn list_prefixed(conn: &Connection, prefix: &str) -> Result<Vec<(String, String)>> {
    let like = format!("{prefix}%");
    let mut stmt = conn.prepare(
        "SELECT key, value FROM setting WHERE key LIKE ?1 ORDER BY key",
    )?;
    let rows = stmt
        .query_map([&like], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_json<T: DeserializeOwned>(conn: &Connection, key: &str) -> Result<Option<T>> {
    match get(conn, key)? {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

pub fn set_json<T: Serialize>(conn: &Connection, key: &str, value: &T) -> Result<()> {
    let s = serde_json::to_string(value)?;
    set(conn, key, &s)
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
    fn get_returns_none_for_unknown_key() {
        let (_d, conn) = fresh_conn();
        assert!(get(&conn, "nope").unwrap().is_none());
    }

    #[test]
    fn set_then_get_roundtrips() {
        let (_d, conn) = fresh_conn();
        set(&conn, "foo", "bar").unwrap();
        assert_eq!(get(&conn, "foo").unwrap(), Some("bar".to_string()));
    }

    #[test]
    fn set_overwrites_existing_key() {
        let (_d, conn) = fresh_conn();
        set(&conn, "foo", "1").unwrap();
        set(&conn, "foo", "2").unwrap();
        assert_eq!(get(&conn, "foo").unwrap(), Some("2".to_string()));
    }

    #[test]
    fn delete_removes_key() {
        let (_d, conn) = fresh_conn();
        set(&conn, "foo", "1").unwrap();
        delete(&conn, "foo").unwrap();
        assert!(get(&conn, "foo").unwrap().is_none());
    }

    #[test]
    fn get_or_default_falls_back() {
        let (_d, conn) = fresh_conn();
        assert_eq!(get_or_default(&conn, "missing", "fallback").unwrap(), "fallback");
        set(&conn, "missing", "real").unwrap();
        assert_eq!(get_or_default(&conn, "missing", "fallback").unwrap(), "real");
    }

    #[test]
    fn list_prefixed_filters_by_prefix() {
        let (_d, conn) = fresh_conn();
        set(&conn, "a.x", "1").unwrap();
        set(&conn, "a.y", "2").unwrap();
        set(&conn, "b.z", "3").unwrap();
        let rows = list_prefixed(&conn, "a.").unwrap();
        assert_eq!(rows, vec![
            ("a.x".to_string(), "1".to_string()),
            ("a.y".to_string(), "2".to_string()),
        ]);
    }

    #[test]
    fn json_helpers_roundtrip() {
        let (_d, conn) = fresh_conn();
        let v = vec![1, 2, 3];
        set_json(&conn, "nums", &v).unwrap();
        let got: Vec<i32> = get_json(&conn, "nums").unwrap().unwrap();
        assert_eq!(got, v);
    }
}
