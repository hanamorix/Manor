//! Approver for `add_chore` proposals.

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};

use crate::assistant::chore;
use crate::assistant::proposal::{AddChoreArgs, AddChoreItem, Status};
use crate::assistant::{Applied, ApplyError};
use crate::person;

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: AddChoreArgs = serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
        field: "diff".into(),
        reason: e.to_string(),
    })?;
    let items = args.into_items();
    if items.is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "items".into(),
            reason: "at least one chore is required".into(),
        });
    }

    let mut applied = 0usize;
    let mut errors = Vec::<ApplyError>::new();

    for (idx, item) in items.into_iter().enumerate() {
        match apply_one(tx, item) {
            Ok(()) => applied += 1,
            Err(err) => errors.push(indexed_error(idx, err)),
        }
    }

    let failed = errors.len();
    let status = match (applied, failed) {
        (0, _) => Status::Rejected,
        (_, 0) => Status::Applied,
        _ => Status::PartiallyApplied,
    };
    persist_outcome(tx, proposal_id, status, &errors)?;

    Ok(Applied {
        proposal_id,
        status,
        items_applied: applied,
        items_failed: failed,
        errors,
    })
}

fn apply_one(tx: &Transaction, item: AddChoreItem) -> Result<(), ApplyError> {
    let title = item.title.trim();
    if title.is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "title".into(),
            reason: "title cannot be empty".into(),
        });
    }

    let first_due = item
        .first_due_ms
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    let names: Vec<String> = item
        .rotation_names
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    let rotation = if names.is_empty() {
        "none"
    } else {
        "round_robin"
    };

    let chore_id = chore::insert(tx, title, &item.emoji, &item.rrule, first_due, rotation)
        .map_err(|e| ApplyError::Internal(format!("chore insert failed: {e}")))?;

    for (position, name) in names.iter().enumerate() {
        let person_id = resolve_or_create_person(tx, name)?;
        chore::insert_rotation_member(tx, chore_id, person_id, position as i32)
            .map_err(|e| ApplyError::Internal(format!("rotation insert failed for {name}: {e}")))?;
    }

    Ok(())
}

fn resolve_or_create_person(tx: &Transaction, name: &str) -> Result<i64, ApplyError> {
    let existing: Option<i64> = tx
        .query_row(
            "SELECT id FROM person WHERE lower(name) = lower(?1) AND deleted_at IS NULL ORDER BY id LIMIT 1",
            [name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| ApplyError::Internal(format!("person lookup failed: {e}")))?;

    if let Some(id) = existing {
        return Ok(id);
    }

    person::insert(tx, name, "member", None, None, None)
        .map(|p| p.id)
        .map_err(|e| ApplyError::Internal(format!("person create failed for {name}: {e}")))
}

fn indexed_error(index: usize, err: ApplyError) -> ApplyError {
    match err {
        ApplyError::InvalidArg { field, reason } => ApplyError::InvalidArg {
            field: format!("items[{index}].{field}"),
            reason,
        },
        other => other,
    }
}

fn persist_outcome(
    tx: &Transaction,
    proposal_id: i64,
    status: Status,
    errors: &[ApplyError],
) -> Result<(), ApplyError> {
    let now = Utc::now().timestamp();
    let applied_at = if matches!(status, Status::Rejected) {
        None
    } else {
        Some(now)
    };
    let errors_json = if errors.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(errors)
                .map_err(|e| ApplyError::Internal(format!("apply errors json: {e}")))?,
        )
    };

    tx.execute(
        "UPDATE proposal SET status = ?1, applied_at = ?2, apply_errors_json = ?3 WHERE id = ?4",
        params![status.as_str(), applied_at, errors_json, proposal_id],
    )
    .map_err(|e| ApplyError::Internal(format!("proposal update failed: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use crate::assistant::proposal::{insert, NewProposal};
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn make_proposal(conn: &Connection, diff: &str) -> i64 {
        insert(
            conn,
            NewProposal {
                kind: "add_chore",
                rationale: "module-test",
                diff_json: diff,
                skill: "rhythm",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_single_chore_inserts_chore_and_marks_applied() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "title": "Do dishes",
            "emoji": "·",
            "rrule": "FREQ=DAILY",
            "first_due_ms": 1_776_259_200_000i64,
            "rotation_names": ["Lewis", "Scarlett"]
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        assert_eq!(applied.items_applied, 1);
        assert_eq!(applied.items_failed, 0);

        let chore_id: i64 = conn
            .query_row("SELECT id FROM chore WHERE title = 'Do dishes'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let rotation_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM rotation WHERE chore_id = ?1",
                [chore_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rotation_count, 2);
    }

    #[test]
    fn approve_bundle_partially_applies_valid_items_and_records_errors() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!([
            {
                "title": "Bins",
                "emoji": "·",
                "rrule": "weekly",
                "first_due_ms": 1_776_259_200_000i64,
                "rotation_names": []
            },
            {
                "title": "",
                "emoji": "·",
                "rrule": "weekly",
                "first_due_ms": 1_776_259_200_000i64,
                "rotation_names": []
            }
        ])
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::PartiallyApplied);
        assert_eq!(applied.items_applied, 1);
        assert_eq!(applied.items_failed, 1);
        assert_eq!(chore::list_all(&conn).unwrap().len(), 1);

        let (status, raw_errors): (String, Option<String>) = conn
            .query_row(
                "SELECT status, apply_errors_json FROM proposal WHERE id = ?1",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "partially_applied");
        assert!(raw_errors.unwrap().contains("items[1].title"));
    }
}
