//! Time blocks — focus/errands/admin/dnd blocks on the calendar.
//! Pattern detection promotes repeated manual entries to recurring blocks.

use anyhow::Result;
use chrono::{DateTime, Datelike, Utc, Weekday};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeBlock {
    pub id: i64,
    pub title: String,
    pub kind: String,
    pub date: i64,
    pub start_time: String,
    pub end_time: String,
    pub rrule: Option<String>,
    pub is_pattern: bool,
    pub pattern_nudge_dismissed_at: Option<i64>,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatternSuggestion {
    pub trigger_id: i64,
    pub kind: String,
    pub start_time: String,
    pub end_time: String,
    pub weekday: String,
    pub count: u32,
}

impl TimeBlock {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            kind: row.get("kind")?,
            date: row.get("date")?,
            start_time: row.get("start_time")?,
            end_time: row.get("end_time")?,
            rrule: row.get("rrule")?,
            is_pattern: row.get::<_, i64>("is_pattern")? != 0,
            pattern_nudge_dismissed_at: row.get("pattern_nudge_dismissed_at")?,
            created_at: row.get("created_at")?,
            deleted_at: row.get("deleted_at")?,
        })
    }
}

const NUDGE_SUPPRESS_WINDOW_MS: i64 = 14 * 86_400_000;
const PATTERN_LOOKBACK_MS: i64 = 42 * 86_400_000; // 6 weeks
const PATTERN_MIN_MATCHES: u32 = 3;

pub fn insert(
    conn: &Connection,
    title: &str,
    kind: &str,
    date_ms: i64,
    start_time: &str,
    end_time: &str,
) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO time_block (title, kind, date, start_time, end_time, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![title, kind, date_ms, start_time, end_time, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<TimeBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], TimeBlock::from_row)?;
    Ok(rows.next().transpose()?)
}

pub fn list_for_date(conn: &Connection, date_ms: i64) -> Result<Vec<TimeBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block
         WHERE deleted_at IS NULL AND date = ?1
         ORDER BY start_time",
    )?;
    let rows = stmt
        .query_map([date_ms], TimeBlock::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_for_week(conn: &Connection, week_start_ms: i64) -> Result<Vec<TimeBlock>> {
    let week_end = week_start_ms + 7 * 86_400_000;
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block
         WHERE deleted_at IS NULL AND date >= ?1 AND date < ?2
         ORDER BY date, start_time",
    )?;
    let rows = stmt
        .query_map(params![week_start_ms, week_end], TimeBlock::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_recurring(conn: &Connection) -> Result<Vec<TimeBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, date, start_time, end_time, rrule, is_pattern,
                pattern_nudge_dismissed_at, created_at, deleted_at
         FROM time_block
         WHERE deleted_at IS NULL AND is_pattern = 1
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], TimeBlock::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn update(
    conn: &Connection,
    id: i64,
    title: &str,
    kind: &str,
    date_ms: i64,
    start_time: &str,
    end_time: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE time_block SET title = ?1, kind = ?2, date = ?3, start_time = ?4, end_time = ?5
         WHERE id = ?6",
        params![title, kind, date_ms, start_time, end_time, id],
    )?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE time_block SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Promote a one-off block to recurring. Sets is_pattern=1 and rrule.
pub fn promote_to_pattern(conn: &Connection, id: i64, rrule: &str) -> Result<()> {
    conn.execute(
        "UPDATE time_block SET is_pattern = 1, rrule = ?1 WHERE id = ?2",
        params![rrule, id],
    )?;
    Ok(())
}

/// Mark that the user dismissed the pattern nudge for a specific block.
pub fn dismiss_pattern_nudge(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE time_block SET pattern_nudge_dismissed_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

fn weekday_name(wd: Weekday) -> &'static str {
    match wd {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// Check whether a newly-inserted block completes a pattern: same `kind`,
/// same `start_time`, same `end_time`, same weekday, at least 3 occurrences
/// (including the trigger) in the last 6 weeks. Suppressed for 14 days after
/// dismissal.
pub fn check_pattern(
    conn: &Connection,
    trigger_id: i64,
    now_ms: i64,
) -> Result<Option<PatternSuggestion>> {
    let trigger = match get(conn, trigger_id)? {
        Some(t) => t,
        None => return Ok(None),
    };
    if trigger.is_pattern {
        return Ok(None);
    }
    if let Some(dismissed_at) = trigger.pattern_nudge_dismissed_at {
        if now_ms - dismissed_at < NUDGE_SUPPRESS_WINDOW_MS {
            return Ok(None);
        }
    }

    let weekday = DateTime::<Utc>::from_timestamp_millis(trigger.date)
        .ok_or_else(|| anyhow::anyhow!("invalid trigger.date"))?
        .weekday();
    let lookback_start = now_ms - PATTERN_LOOKBACK_MS;

    let mut stmt = conn.prepare(
        "SELECT id, date FROM time_block
         WHERE deleted_at IS NULL
           AND is_pattern = 0
           AND kind = ?1 AND start_time = ?2 AND end_time = ?3
           AND date >= ?4",
    )?;
    let rows = stmt.query_map(
        params![
            trigger.kind,
            trigger.start_time,
            trigger.end_time,
            lookback_start,
        ],
        |r| {
            let id: i64 = r.get("id")?;
            let date: i64 = r.get("date")?;
            Ok((id, date))
        },
    )?;

    let mut count: u32 = 0;
    for row in rows {
        let (_id, date) = row?;
        let wd = DateTime::<Utc>::from_timestamp_millis(date)
            .ok_or_else(|| anyhow::anyhow!("invalid row date"))?
            .weekday();
        if wd == weekday {
            count += 1;
        }
    }
    if count >= PATTERN_MIN_MATCHES {
        Ok(Some(PatternSuggestion {
            trigger_id,
            kind: trigger.kind,
            start_time: trigger.start_time,
            end_time: trigger.end_time,
            weekday: weekday_name(weekday).to_string(),
            count,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn day_ms(y: i32, m: u32, d: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp_millis()
    }

    #[test]
    fn insert_and_get_round_trip() {
        let (_d, conn) = fresh_conn();
        let date = day_ms(2026, 4, 15);
        let id = insert(&conn, "Deep work", "focus", date, "09:00", "11:00").unwrap();
        let got = get(&conn, id).unwrap().unwrap();
        assert_eq!(got.title, "Deep work");
        assert_eq!(got.kind, "focus");
        assert!(!got.is_pattern);
    }

    #[test]
    fn list_for_date_filters_correctly() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "A", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        insert(&conn, "B", "admin", day_ms(2026, 4, 15), "14:00", "15:00").unwrap();
        insert(&conn, "C", "focus", day_ms(2026, 4, 16), "09:00", "11:00").unwrap();

        let today = list_for_date(&conn, day_ms(2026, 4, 15)).unwrap();
        assert_eq!(today.len(), 2);
    }

    #[test]
    fn soft_delete_hides_from_lists() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "A", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        soft_delete(&conn, id).unwrap();
        assert_eq!(list_for_date(&conn, day_ms(2026, 4, 15)).unwrap().len(), 0);
    }

    #[test]
    fn promote_to_pattern_sets_flags() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "A", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        promote_to_pattern(&conn, id, "FREQ=WEEKLY;BYDAY=TU").unwrap();
        let got = get(&conn, id).unwrap().unwrap();
        assert!(got.is_pattern);
        assert_eq!(got.rrule.as_deref(), Some("FREQ=WEEKLY;BYDAY=TU"));
    }

    #[test]
    fn check_pattern_fires_when_three_same_weekday_matches() {
        let (_d, conn) = fresh_conn();
        // Three consecutive Tuesdays
        insert(&conn, "Focus", "focus", day_ms(2026, 3, 31), "09:00", "11:00").unwrap();
        insert(&conn, "Focus", "focus", day_ms(2026, 4, 7), "09:00", "11:00").unwrap();
        let trigger = insert(&conn, "Focus", "focus", day_ms(2026, 4, 14), "09:00", "11:00").unwrap();

        let now = day_ms(2026, 4, 14) + 3_600_000;
        let sugg = check_pattern(&conn, trigger, now).unwrap();
        let s = sugg.expect("pattern suggestion expected");
        assert_eq!(s.weekday, "Tuesday");
        assert!(s.count >= 3);
    }

    #[test]
    fn check_pattern_suppresses_when_dismissed_recently() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "Focus", "focus", day_ms(2026, 3, 31), "09:00", "11:00").unwrap();
        insert(&conn, "Focus", "focus", day_ms(2026, 4, 7), "09:00", "11:00").unwrap();
        let trigger = insert(&conn, "Focus", "focus", day_ms(2026, 4, 14), "09:00", "11:00").unwrap();
        dismiss_pattern_nudge(&conn, trigger).unwrap();

        let now = day_ms(2026, 4, 14) + 3_600_000;
        assert!(check_pattern(&conn, trigger, now).unwrap().is_none());
    }

    #[test]
    fn check_pattern_no_match_when_weekdays_differ() {
        let (_d, conn) = fresh_conn();
        insert(&conn, "Focus", "focus", day_ms(2026, 3, 31), "09:00", "11:00").unwrap(); // Tue
        insert(&conn, "Focus", "focus", day_ms(2026, 4, 1), "09:00", "11:00").unwrap(); // Wed
        let trigger = insert(&conn, "Focus", "focus", day_ms(2026, 4, 14), "09:00", "11:00").unwrap();

        let now = day_ms(2026, 4, 14) + 3_600_000;
        // Only trigger (Tue) + 3/31 (Tue) = 2, < 3.
        assert!(check_pattern(&conn, trigger, now).unwrap().is_none());
    }

    #[test]
    fn list_recurring_only_patterns() {
        let (_d, conn) = fresh_conn();
        let a = insert(&conn, "One-off", "focus", day_ms(2026, 4, 15), "09:00", "11:00").unwrap();
        let b = insert(&conn, "Weekly", "focus", day_ms(2026, 4, 15), "14:00", "15:00").unwrap();
        promote_to_pattern(&conn, b, "FREQ=WEEKLY;BYDAY=TU").unwrap();

        let patterns = list_recurring(&conn).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].id, b);
        let _ = a;
    }
}
