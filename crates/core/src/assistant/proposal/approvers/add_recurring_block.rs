//! Approver for `add_recurring_block` proposals.

use chrono::Utc;
use rusqlite::{params, Transaction};

use crate::assistant::proposal::{AddRecurringBlockArgs, Status};
use crate::assistant::time_block;
use crate::assistant::{Applied, ApplyError};

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: AddRecurringBlockArgs =
        serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
            field: "diff".into(),
            reason: e.to_string(),
        })?;
    validate(&args)?;

    let block_id = time_block::insert(
        tx,
        args.title.trim(),
        args.kind.trim(),
        args.date_ms,
        args.start_time.trim(),
        args.end_time.trim(),
    )
    .map_err(|e| ApplyError::Internal(format!("time block insert failed: {e}")))?;
    time_block::promote_to_pattern(tx, block_id, args.rrule.trim())
        .map_err(|e| ApplyError::Internal(format!("recurring block promote failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn validate(args: &AddRecurringBlockArgs) -> Result<(), ApplyError> {
    if args.title.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "title".into(),
            reason: "title cannot be empty".into(),
        });
    }
    if args.kind.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "kind".into(),
            reason: "kind cannot be empty".into(),
        });
    }
    if args.rrule.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "rrule".into(),
            reason: "rrule cannot be empty".into(),
        });
    }
    if !looks_like_hhmm(&args.start_time) {
        return Err(ApplyError::InvalidArg {
            field: "start_time".into(),
            reason: "expected HH:MM".into(),
        });
    }
    if !looks_like_hhmm(&args.end_time) {
        return Err(ApplyError::InvalidArg {
            field: "end_time".into(),
            reason: "expected HH:MM".into(),
        });
    }
    if args.start_time >= args.end_time {
        return Err(ApplyError::InvalidArg {
            field: "end_time".into(),
            reason: "end_time must be after start_time".into(),
        });
    }
    Ok(())
}

fn looks_like_hhmm(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return false;
    }
    if !bytes[0].is_ascii_digit()
        || !bytes[1].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
        || !bytes[4].is_ascii_digit()
    {
        return false;
    }
    let hour = (bytes[0] - b'0') * 10 + (bytes[1] - b'0');
    let minute = (bytes[3] - b'0') * 10 + (bytes[4] - b'0');
    hour < 24 && minute < 60
}

fn mark_applied(tx: &Transaction, proposal_id: i64) -> Result<(), ApplyError> {
    let now = Utc::now().timestamp();
    tx.execute(
        "UPDATE proposal SET status = ?1, applied_at = ?2, apply_errors_json = NULL WHERE id = ?3",
        params![Status::Applied.as_str(), now, proposal_id],
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
                kind: "add_recurring_block",
                rationale: "module-test",
                diff_json: diff,
                skill: "rhythm",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_inserts_pattern_time_block() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "title": "Planning",
            "kind": "admin",
            "date_ms": 1_777_132_800_000i64,
            "start_time": "09:00",
            "end_time": "09:30",
            "rrule": "FREQ=WEEKLY;BYDAY=MO"
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.items_applied, 1);
        let (is_pattern, rrule): (i64, String) = conn
            .query_row(
                "SELECT is_pattern, rrule FROM time_block WHERE title = 'Planning'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(is_pattern, 1);
        assert_eq!(rrule, "FREQ=WEEKLY;BYDAY=MO");
    }
}
