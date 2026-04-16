//! Events — calendar entries synced from CalDAV.
//!
//! Sync strategy is wipe-and-reinsert per account (no incremental in v0.1),
//! so `insert_many` is batched and `delete_for_account` is called at the
//! start of every sync.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub id: i64,
    pub calendar_account_id: i64,
    pub external_id: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub created_at: i64,
    pub event_url: Option<String>,
    pub etag: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
    pub is_recurring_occurrence: bool,
    pub parent_event_url: Option<String>,
    pub occurrence_dtstart: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub calendar_account_id: i64,
    pub external_id: String,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub event_url: Option<String>,
    pub etag: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
    pub is_recurring_occurrence: bool,
    pub parent_event_url: Option<String>,
    pub occurrence_dtstart: Option<String>,
}

impl Event {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            calendar_account_id: row.get("calendar_account_id")?,
            external_id: row.get("external_id")?,
            title: row.get("title")?,
            start_at: row.get("start_at")?,
            end_at: row.get("end_at")?,
            created_at: row.get("created_at")?,
            event_url: row.get("event_url")?,
            etag: row.get("etag")?,
            description: row.get("description")?,
            location: row.get("location")?,
            all_day: row.get::<_, i64>("all_day").map(|v| v != 0)?,
            is_recurring_occurrence: row.get::<_, i64>("is_recurring_occurrence").map(|v| v != 0)?,
            parent_event_url: row.get("parent_event_url")?,
            occurrence_dtstart: row.get("occurrence_dtstart")?,
        })
    }
}

/// Batch-insert events. Single transaction — all-or-nothing.
pub fn insert_many(conn: &Connection, events: &[NewEvent]) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let now_ms = Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO event (
                calendar_account_id, external_id, title, start_at, end_at, created_at,
                event_url, etag, description, location, all_day,
                is_recurring_occurrence, parent_event_url, occurrence_dtstart
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;
        for ev in events {
            stmt.execute(params![
                ev.calendar_account_id,
                ev.external_id,
                ev.title,
                ev.start_at,
                ev.end_at,
                now_ms,
                ev.event_url,
                ev.etag,
                ev.description,
                ev.location,
                ev.all_day as i64,
                ev.is_recurring_occurrence as i64,
                ev.parent_event_url,
                ev.occurrence_dtstart,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Events whose start_at falls in `[start_utc, end_utc)`. Ordered by start_at.
pub fn list_today(conn: &Connection, start_utc: i64, end_utc: i64) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, calendar_account_id, external_id, title, start_at, end_at, created_at,
                event_url, etag, description, location, all_day,
                is_recurring_occurrence, parent_event_url, occurrence_dtstart
         FROM event
         WHERE start_at >= ?1 AND start_at < ?2 AND deleted_at IS NULL
         ORDER BY start_at",
    )?;
    let rows = stmt
        .query_map(params![start_utc, end_utc], Event::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Soft-delete an event by setting `deleted_at` to the current Unix timestamp.
pub fn soft_delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    let affected = conn.execute(
        "UPDATE event SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if affected == 0 {
        anyhow::bail!("event {id} not found");
    }
    Ok(())
}

pub fn delete_for_account(conn: &Connection, account_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM event WHERE calendar_account_id = ?1",
        [account_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::{calendar_account, db};
    use tempfile::tempdir;

    fn fresh_conn_with_account() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        let id = calendar_account::insert(&conn, "iCloud", "https://caldav.icloud.com", "a@b.c")
            .unwrap();
        (dir, conn, id)
    }

    #[test]
    fn insert_many_persists_batch() {
        let (_d, conn, aid) = fresh_conn_with_account();
        let batch = vec![
            NewEvent {
                calendar_account_id: aid,
                external_id: "u1".into(),
                title: "A".into(),
                start_at: 100,
                end_at: 200,
                event_url: None,
                etag: None,
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: false,
                parent_event_url: None,
                occurrence_dtstart: None,
            },
            NewEvent {
                calendar_account_id: aid,
                external_id: "u2".into(),
                title: "B".into(),
                start_at: 300,
                end_at: 400,
                event_url: None,
                etag: None,
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: false,
                parent_event_url: None,
                occurrence_dtstart: None,
            },
        ];
        insert_many(&conn, &batch).unwrap();
        let rows = list_today(&conn, 0, 1000).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn list_today_filters_by_utc_bounds() {
        let (_d, conn, aid) = fresh_conn_with_account();
        insert_many(
            &conn,
            &[
                NewEvent {
                    calendar_account_id: aid,
                    external_id: "u1".into(),
                    title: "yesterday".into(),
                    start_at: 50,
                    end_at: 99,
                    event_url: None,
                    etag: None,
                    description: None,
                    location: None,
                    all_day: false,
                    is_recurring_occurrence: false,
                    parent_event_url: None,
                    occurrence_dtstart: None,
                },
                NewEvent {
                    calendar_account_id: aid,
                    external_id: "u2".into(),
                    title: "today".into(),
                    start_at: 150,
                    end_at: 200,
                    event_url: None,
                    etag: None,
                    description: None,
                    location: None,
                    all_day: false,
                    is_recurring_occurrence: false,
                    parent_event_url: None,
                    occurrence_dtstart: None,
                },
                NewEvent {
                    calendar_account_id: aid,
                    external_id: "u3".into(),
                    title: "tomorrow".into(),
                    start_at: 500,
                    end_at: 600,
                    event_url: None,
                    etag: None,
                    description: None,
                    location: None,
                    all_day: false,
                    is_recurring_occurrence: false,
                    parent_event_url: None,
                    occurrence_dtstart: None,
                },
            ],
        )
        .unwrap();
        let rows = list_today(&conn, 100, 300).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "today");
    }

    #[test]
    fn delete_for_account_scoped_to_account() {
        let (_d, conn, aid) = fresh_conn_with_account();
        let other = calendar_account::insert(&conn, "Other", "https://x.test", "x").unwrap();
        insert_many(
            &conn,
            &[
                NewEvent {
                    calendar_account_id: aid,
                    external_id: "a1".into(),
                    title: "A".into(),
                    start_at: 1,
                    end_at: 2,
                    event_url: None,
                    etag: None,
                    description: None,
                    location: None,
                    all_day: false,
                    is_recurring_occurrence: false,
                    parent_event_url: None,
                    occurrence_dtstart: None,
                },
                NewEvent {
                    calendar_account_id: other,
                    external_id: "o1".into(),
                    title: "O".into(),
                    start_at: 1,
                    end_at: 2,
                    event_url: None,
                    etag: None,
                    description: None,
                    location: None,
                    all_day: false,
                    is_recurring_occurrence: false,
                    parent_event_url: None,
                    occurrence_dtstart: None,
                },
            ],
        )
        .unwrap();

        delete_for_account(&conn, aid).unwrap();

        let rows = list_today(&conn, 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "O");
    }

    #[test]
    fn insert_many_empty_is_noop() {
        let (_d, conn, _aid) = fresh_conn_with_account();
        insert_many(&conn, &[]).unwrap();
        let rows = list_today(&conn, 0, 1_000_000).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn list_today_excludes_deleted_events() {
        let (_d, conn, aid) = fresh_conn_with_account();
        insert_many(
            &conn,
            &[NewEvent {
                calendar_account_id: aid,
                external_id: "ev1".into(),
                title: "Alive".into(),
                start_at: 150,
                end_at: 200,
                event_url: None,
                etag: None,
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: false,
                parent_event_url: None,
                occurrence_dtstart: None,
            }],
        )
        .unwrap();
        let id = list_today(&conn, 0, 1000).unwrap()[0].id;
        soft_delete(&conn, id).unwrap();
        let rows = list_today(&conn, 0, 1000).unwrap();
        assert_eq!(rows.len(), 0, "deleted events must be hidden");
    }

    #[test]
    fn soft_delete_sets_deleted_at() {
        let (_d, conn, aid) = fresh_conn_with_account();
        insert_many(
            &conn,
            &[NewEvent {
                calendar_account_id: aid,
                external_id: "del1".into(),
                title: "Gone".into(),
                start_at: 150,
                end_at: 200,
                event_url: Some("https://cal.example.com/del1.ics".into()),
                etag: Some("\"abc123\"".into()),
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: false,
                parent_event_url: None,
                occurrence_dtstart: None,
            }],
        )
        .unwrap();
        let id = list_today(&conn, 0, 1000).unwrap()[0].id;
        soft_delete(&conn, id).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE id = ?1 AND deleted_at IS NOT NULL",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
}
