//! Chores — recurring household tasks with rotation support.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rrule::{RRuleSet, Tz as RruleTz};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chore {
    pub id: i64,
    pub title: String,
    pub emoji: String,
    pub rrule: String,
    pub next_due: i64,
    pub rotation: String,
    pub active: bool,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChoreCompletion {
    pub id: i64,
    pub chore_id: i64,
    pub completed_at: i64,
    pub completed_by: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RotationMember {
    pub id: i64,
    pub chore_id: i64,
    pub person_id: i64,
    pub position: i32,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FairnessNudge {
    pub chore_id: i64,
    pub chore_title: String,
    pub person_id: i64,
    pub person_name: String,
    pub days_ago: u32,
}

impl Chore {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            emoji: row.get("emoji")?,
            rrule: row.get("rrule")?,
            next_due: row.get("next_due")?,
            rotation: row.get("rotation")?,
            active: row.get::<_, i64>("active")? != 0,
            created_at: row.get("created_at")?,
            deleted_at: row.get("deleted_at")?,
        })
    }
}

impl ChoreCompletion {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            chore_id: row.get("chore_id")?,
            completed_at: row.get("completed_at")?,
            completed_by: row.get("completed_by")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl RotationMember {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            chore_id: row.get("chore_id")?,
            person_id: row.get("person_id")?,
            position: row.get("position")?,
            current: row.get::<_, i64>("current")? != 0,
        })
    }
}

/// Compute the next occurrence of an RRULE strictly after `after_ms`.
/// Used to advance `next_due` after completion or skip.
pub fn next_occurrence_after(rrule_str: &str, after_ms: i64) -> Result<i64> {
    let after_secs = after_ms / 1000;
    let sub_nanos = ((after_ms % 1000) * 1_000_000).max(0) as u32;
    let after_dt = DateTime::<Utc>::from_timestamp(after_secs, sub_nanos)
        .ok_or_else(|| anyhow::anyhow!("invalid after_ms"))?;
    let rule_block = format!(
        "DTSTART:{}\nRRULE:{}",
        after_dt.format("%Y%m%dT%H%M%SZ"),
        rrule_str
    );
    let rset = RRuleSet::from_str(&rule_block)?;
    let after_rrule = after_dt.with_timezone(&RruleTz::UTC);
    let result = rset.after(after_rrule).all(2);
    let occ = result
        .dates
        .into_iter()
        .find(|d| d.timestamp_millis() > after_ms)
        .ok_or_else(|| anyhow::anyhow!("no next occurrence"))?;
    Ok(occ.with_timezone(&Utc).timestamp_millis())
}

/// Insert a person (minimal household member row). Returns new row id.
pub fn insert_person(conn: &Connection, name: &str) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO person (name, created_at) VALUES (?1, ?2)",
        params![name, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Create a new chore. `rrule` is an RFC 5545 RRULE string (without the
/// `RRULE:` prefix), e.g. `FREQ=WEEKLY`. `first_due` is the initial next_due
/// timestamp in unix ms.
pub fn insert(
    conn: &Connection,
    title: &str,
    emoji: &str,
    rrule: &str,
    first_due: i64,
    rotation: &str,
) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO chore (title, emoji, rrule, next_due, rotation, active, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
        params![title, emoji, rrule, first_due, rotation, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Chores whose next_due is ≤ end_of_today_ms, active, not deleted.
pub fn list_due_today(conn: &Connection, end_of_today_ms: i64) -> Result<Vec<Chore>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, emoji, rrule, next_due, rotation, active, created_at, deleted_at
         FROM chore
         WHERE active = 1 AND deleted_at IS NULL AND next_due <= ?1
         ORDER BY next_due",
    )?;
    let rows = stmt
        .query_map([end_of_today_ms], Chore::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// All active, non-deleted chores, sorted by next_due.
pub fn list_all(conn: &Connection) -> Result<Vec<Chore>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, emoji, rrule, next_due, rotation, active, created_at, deleted_at
         FROM chore
         WHERE deleted_at IS NULL
         ORDER BY next_due",
    )?;
    let rows = stmt
        .query_map([], Chore::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Fetch a single chore by id (including soft-deleted).
pub fn get(conn: &Connection, id: i64) -> Result<Option<Chore>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, emoji, rrule, next_due, rotation, active, created_at, deleted_at
         FROM chore WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], Chore::from_row)?;
    Ok(rows.next().transpose()?)
}

pub fn update(
    conn: &Connection,
    id: i64,
    title: &str,
    emoji: &str,
    rrule: &str,
    rotation: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE chore SET title = ?1, emoji = ?2, rrule = ?3, rotation = ?4
         WHERE id = ?5",
        params![title, emoji, rrule, rotation, id],
    )?;
    Ok(())
}

/// Soft-delete a chore.
pub fn soft_delete(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE chore SET deleted_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Complete a chore: record a chore_completion, advance next_due using the
/// original next_due as the base (predictable schedule), advance rotation.
pub fn complete(conn: &Connection, chore_id: i64, completed_by: Option<i64>) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let chore =
        get(conn, chore_id)?.ok_or_else(|| anyhow::anyhow!("chore {chore_id} not found"))?;

    conn.execute(
        "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![chore_id, now, completed_by, now],
    )?;

    let next = next_occurrence_after(&chore.rrule, chore.next_due)?;
    conn.execute(
        "UPDATE chore SET next_due = ?1 WHERE id = ?2",
        params![next, chore_id],
    )?;

    if chore.rotation == "round_robin" {
        advance_rotation(conn, chore_id)?;
    }
    Ok(())
}

/// Skip a chore: advance next_due without recording a completion.
pub fn skip(conn: &Connection, chore_id: i64) -> Result<()> {
    let chore =
        get(conn, chore_id)?.ok_or_else(|| anyhow::anyhow!("chore {chore_id} not found"))?;
    let next = next_occurrence_after(&chore.rrule, chore.next_due)?;
    conn.execute(
        "UPDATE chore SET next_due = ?1 WHERE id = ?2",
        params![next, chore_id],
    )?;
    Ok(())
}

/// List the last N completions for a chore, newest first.
pub fn list_completions(
    conn: &Connection,
    chore_id: i64,
    limit: u32,
) -> Result<Vec<ChoreCompletion>> {
    let mut stmt = conn.prepare(
        "SELECT id, chore_id, completed_at, completed_by, created_at
         FROM chore_completion
         WHERE chore_id = ?1
         ORDER BY completed_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![chore_id, limit], ChoreCompletion::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Insert a rotation member for a chore at the given position. The first
/// member (position 0) is current by default.
pub fn insert_rotation_member(
    conn: &Connection,
    chore_id: i64,
    person_id: i64,
    position: i32,
) -> Result<i64> {
    let now = Utc::now().timestamp_millis();
    let current = if position == 0 { 1 } else { 0 };
    conn.execute(
        "INSERT INTO rotation (chore_id, person_id, position, current, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chore_id, person_id, position, current, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Fetch rotation members for a chore, ordered by position.
pub fn list_rotation(conn: &Connection, chore_id: i64) -> Result<Vec<RotationMember>> {
    let mut stmt = conn.prepare(
        "SELECT id, chore_id, person_id, position, current
         FROM rotation WHERE chore_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([chore_id], RotationMember::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Advance the current pointer in a round-robin rotation to the next position,
/// wrapping around.
pub fn advance_rotation(conn: &Connection, chore_id: i64) -> Result<()> {
    let members = list_rotation(conn, chore_id)?;
    if members.len() <= 1 {
        return Ok(());
    }
    let current_idx = members.iter().position(|m| m.current).unwrap_or(0);
    let next_idx = (current_idx + 1) % members.len();

    conn.execute(
        "UPDATE rotation SET current = 0 WHERE id = ?1",
        [members[current_idx].id],
    )?;
    conn.execute(
        "UPDATE rotation SET current = 1 WHERE id = ?1",
        [members[next_idx].id],
    )?;
    Ok(())
}

/// For each active chore with a rotation, flag the assignee whose days-since-
/// last-completion is > 2× the group median (with a 7-day floor to avoid
/// noise when everyone's recent). Returns at most one `FairnessNudge` per chore.
pub fn check_fairness(conn: &Connection, now_ms: i64) -> Result<Vec<FairnessNudge>> {
    let mut chore_stmt = conn.prepare(
        "SELECT c.id, c.title FROM chore c
         WHERE c.active = 1 AND c.deleted_at IS NULL AND c.rotation != 'none'",
    )?;
    let chores: Vec<(i64, String)> = chore_stmt
        .query_map([], |r| Ok((r.get("id")?, r.get("title")?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut nudges = Vec::new();
    for (chore_id, chore_title) in chores {
        let mut member_stmt = conn.prepare(
            "SELECT r.person_id AS pid, p.name AS name,
                    (SELECT MAX(cc.completed_at) FROM chore_completion cc
                     WHERE cc.chore_id = r.chore_id AND cc.completed_by = r.person_id) AS last_ms
             FROM rotation r
             JOIN person p ON p.id = r.person_id
             WHERE r.chore_id = ?1",
        )?;
        let members: Vec<(i64, String, Option<i64>)> = member_stmt
            .query_map([chore_id], |r| {
                Ok((r.get("pid")?, r.get("name")?, r.get("last_ms")?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        if members.len() < 2 {
            continue;
        }

        let days: Vec<(i64, String, u32)> = members
            .into_iter()
            .map(|(pid, name, last)| {
                let d = match last {
                    Some(ms) => ((now_ms - ms) / 86_400_000).max(0) as u32,
                    None => 9999,
                };
                (pid, name, d)
            })
            .collect();

        let mut days_sorted: Vec<u32> = days.iter().map(|(_, _, d)| *d).collect();
        days_sorted.sort_unstable();
        // Lower-median: for even-length arrays use the smaller of the two middle
        // values, so a single heavy outlier still crosses the 2× threshold.
        let median_idx = (days_sorted.len() - 1) / 2;
        let median = days_sorted[median_idx];
        let threshold = (median.saturating_mul(2)).max(7);

        if let Some((pid, name, d)) = days
            .into_iter()
            .filter(|(_, _, d)| *d > threshold)
            .max_by_key(|(_, _, d)| *d)
        {
            nudges.push(FairnessNudge {
                chore_id,
                chore_title: chore_title.clone(),
                person_id: pid,
                person_name: name,
                days_ago: d.min(9998),
            });
        }
    }
    Ok(nudges)
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
    fn insert_returns_id_and_persists() {
        let (_d, conn) = fresh_conn();
        let due = Utc::now().timestamp_millis();
        let id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", due, "none").unwrap();
        assert!(id > 0);
        let got = get(&conn, id).unwrap().unwrap();
        assert_eq!(got.title, "Bins");
        assert_eq!(got.rotation, "none");
        assert!(got.active);
    }

    #[test]
    fn list_due_today_includes_past_and_today_excludes_future() {
        let (_d, conn) = fresh_conn();
        let now = Utc::now().timestamp_millis();
        let end_of_today = now + 3_600_000;
        insert(
            &conn,
            "overdue",
            "🧹",
            "FREQ=WEEKLY",
            now - 86_400_000,
            "none",
        )
        .unwrap();
        insert(&conn, "today", "🧹", "FREQ=WEEKLY", now, "none").unwrap();
        insert(
            &conn,
            "later",
            "🧹",
            "FREQ=WEEKLY",
            now + 7 * 86_400_000,
            "none",
        )
        .unwrap();

        let due = list_due_today(&conn, end_of_today).unwrap();
        let titles: Vec<&str> = due.iter().map(|c| c.title.as_str()).collect();
        assert!(titles.contains(&"overdue"));
        assert!(titles.contains(&"today"));
        assert!(!titles.contains(&"later"));
    }

    #[test]
    fn soft_delete_excludes_from_list() {
        let (_d, conn) = fresh_conn();
        let now = Utc::now().timestamp_millis();
        let id = insert(&conn, "Gone", "🧹", "FREQ=WEEKLY", now, "none").unwrap();
        soft_delete(&conn, id).unwrap();
        assert_eq!(list_all(&conn).unwrap().len(), 0);
        assert!(get(&conn, id).unwrap().unwrap().deleted_at.is_some());
    }

    #[test]
    fn next_occurrence_after_weekly_advances_seven_days() {
        let base = 1_776_259_200_000i64;
        let next = next_occurrence_after("FREQ=WEEKLY", base).unwrap();
        assert_eq!(next, base + 7 * 86_400_000);
    }

    #[test]
    fn complete_inserts_completion_and_advances_next_due() {
        let (_d, conn) = fresh_conn();
        let start = 1_776_259_200_000i64;
        let id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "none").unwrap();
        complete(&conn, id, None).unwrap();

        let updated = get(&conn, id).unwrap().unwrap();
        assert_eq!(updated.next_due, start + 7 * 86_400_000);

        let comps = list_completions(&conn, id, 10).unwrap();
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].chore_id, id);
        assert!(comps[0].completed_by.is_none());
    }

    #[test]
    fn complete_with_person_records_completed_by() {
        let (_d, conn) = fresh_conn();
        let pid = insert_person(&conn, "Rosa").unwrap();
        let id = insert(
            &conn,
            "Bins",
            "🗑️",
            "FREQ=WEEKLY",
            1_776_259_200_000,
            "none",
        )
        .unwrap();
        complete(&conn, id, Some(pid)).unwrap();

        let comps = list_completions(&conn, id, 10).unwrap();
        assert_eq!(comps[0].completed_by, Some(pid));
    }

    #[test]
    fn skip_advances_next_due_without_completion() {
        let (_d, conn) = fresh_conn();
        let start = 1_776_259_200_000i64;
        let id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "none").unwrap();
        skip(&conn, id).unwrap();

        let updated = get(&conn, id).unwrap().unwrap();
        assert_eq!(updated.next_due, start + 7 * 86_400_000);
        assert_eq!(list_completions(&conn, id, 10).unwrap().len(), 0);
    }

    #[test]
    fn round_robin_rotation_advances_on_complete() {
        let (_d, conn) = fresh_conn();
        let a = insert_person(&conn, "A").unwrap();
        let b = insert_person(&conn, "B").unwrap();
        let c = insert_person(&conn, "C").unwrap();
        let chore_id = insert(
            &conn,
            "Bins",
            "🗑️",
            "FREQ=WEEKLY",
            1_776_259_200_000,
            "round_robin",
        )
        .unwrap();
        insert_rotation_member(&conn, chore_id, a, 0).unwrap();
        insert_rotation_member(&conn, chore_id, b, 1).unwrap();
        insert_rotation_member(&conn, chore_id, c, 2).unwrap();

        let members = list_rotation(&conn, chore_id).unwrap();
        assert!(members[0].current);

        complete(&conn, chore_id, Some(a)).unwrap();
        let after_one = list_rotation(&conn, chore_id).unwrap();
        assert!(!after_one[0].current);
        assert!(after_one[1].current);

        complete(&conn, chore_id, Some(b)).unwrap();
        complete(&conn, chore_id, Some(c)).unwrap();
        let after_three = list_rotation(&conn, chore_id).unwrap();
        assert!(after_three[0].current);
    }

    #[test]
    fn update_changes_fields() {
        let (_d, conn) = fresh_conn();
        let id = insert(&conn, "Old", "🧹", "FREQ=WEEKLY", 1_776_259_200_000, "none").unwrap();
        update(&conn, id, "New", "🧽", "FREQ=DAILY", "round_robin").unwrap();
        let got = get(&conn, id).unwrap().unwrap();
        assert_eq!(got.title, "New");
        assert_eq!(got.emoji, "🧽");
        assert_eq!(got.rrule, "FREQ=DAILY");
        assert_eq!(got.rotation, "round_robin");
    }

    #[test]
    fn check_fairness_flags_single_outlier() {
        let (_d, conn) = fresh_conn();
        let a = insert_person(&conn, "A").unwrap();
        let b = insert_person(&conn, "B").unwrap();

        let start = 1_776_259_200_000i64;
        let chore_id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "round_robin").unwrap();
        insert_rotation_member(&conn, chore_id, a, 0).unwrap();
        insert_rotation_member(&conn, chore_id, b, 1).unwrap();

        let now = start + 30 * 86_400_000;
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 86_400_000, a],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 30 * 86_400_000, b],
        )
        .unwrap();

        let nudges = check_fairness(&conn, now).unwrap();
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].person_name, "B");
    }

    #[test]
    fn check_fairness_empty_when_distribution_even() {
        let (_d, conn) = fresh_conn();
        let a = insert_person(&conn, "A").unwrap();
        let b = insert_person(&conn, "B").unwrap();
        let start = 1_776_259_200_000i64;
        let chore_id = insert(&conn, "Bins", "🗑️", "FREQ=WEEKLY", start, "round_robin").unwrap();
        insert_rotation_member(&conn, chore_id, a, 0).unwrap();
        insert_rotation_member(&conn, chore_id, b, 1).unwrap();

        let now = start;
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 2 * 86_400_000, a],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chore_completion (chore_id, completed_at, completed_by, created_at)
             VALUES (?1, ?2, ?3, ?2)",
            params![chore_id, now - 3 * 86_400_000, b],
        )
        .unwrap();

        let nudges = check_fairness(&conn, now).unwrap();
        assert!(nudges.is_empty());
    }
}
