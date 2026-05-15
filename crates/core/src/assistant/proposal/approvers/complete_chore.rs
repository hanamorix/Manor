//! Approver for `complete_chore` proposals.

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};

use crate::assistant::chore;
use crate::assistant::proposal::{CompleteChoreArgs, Status};
use crate::assistant::{Applied, ApplyError};

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: CompleteChoreArgs =
        serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
            field: "diff".into(),
            reason: e.to_string(),
        })?;

    let chore_id = resolve_chore(tx, args.chore_id, args.title.as_deref())?;
    let completed_by = resolve_person(tx, args.completed_by, args.completed_by_name.as_deref())?;

    chore::complete(tx, chore_id, completed_by)
        .map_err(|e| ApplyError::Internal(format!("chore complete failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn resolve_chore(
    tx: &Transaction,
    chore_id: Option<i64>,
    title: Option<&str>,
) -> Result<i64, ApplyError> {
    if let Some(id) = chore_id {
        let exists: bool = tx
            .query_row(
                "SELECT 1 FROM chore WHERE id = ?1 AND active = 1 AND deleted_at IS NULL",
                [id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| ApplyError::Internal(format!("chore lookup failed: {e}")))?
            .unwrap_or(false);
        return if exists {
            Ok(id)
        } else {
            Err(ApplyError::StaleReference {
                entity: "chore".into(),
                id: id.to_string(),
            })
        };
    }

    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .ok_or_else(|| ApplyError::InvalidArg {
            field: "title".into(),
            reason: "title is required when chore_id is omitted".into(),
        })?;

    let matches: Vec<i64> = {
        let mut stmt = tx
            .prepare(
                "SELECT id FROM chore
                 WHERE active = 1 AND deleted_at IS NULL AND lower(title) = lower(?1)
                 ORDER BY id",
            )
            .map_err(|e| ApplyError::Internal(format!("chore lookup failed: {e}")))?;
        let rows = stmt
            .query_map([title], |row| row.get(0))
            .map_err(|e| ApplyError::Internal(format!("chore lookup failed: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApplyError::Internal(format!("chore lookup failed: {e}")))?;
        rows
    };

    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(ApplyError::StaleReference {
            entity: "chore".into(),
            id: title.to_string(),
        }),
        _ => Err(ApplyError::Conflict(format!(
            "multiple active chores match title '{title}'"
        ))),
    }
}

fn resolve_person(
    tx: &Transaction,
    completed_by: Option<i64>,
    completed_by_name: Option<&str>,
) -> Result<Option<i64>, ApplyError> {
    if let Some(id) = completed_by {
        let exists: bool = tx
            .query_row(
                "SELECT 1 FROM person WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| ApplyError::Internal(format!("person lookup failed: {e}")))?
            .unwrap_or(false);
        return if exists {
            Ok(Some(id))
        } else {
            Err(ApplyError::StaleReference {
                entity: "person".into(),
                id: id.to_string(),
            })
        };
    }

    let Some(name) = completed_by_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return Ok(None);
    };

    let id: Option<i64> = tx
        .query_row(
            "SELECT id FROM person WHERE lower(name) = lower(?1) AND deleted_at IS NULL ORDER BY id LIMIT 1",
            [name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| ApplyError::Internal(format!("person lookup failed: {e}")))?;
    id.map(Some).ok_or_else(|| ApplyError::StaleReference {
        entity: "person".into(),
        id: name.to_string(),
    })
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
                kind: "complete_chore",
                rationale: "module-test",
                diff_json: diff,
                skill: "rhythm",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_resolves_chore_by_title_and_records_completion() {
        let (_d, mut conn) = fresh_conn();
        let chore_id = chore::insert(
            &conn,
            "Do dishes",
            ".",
            "FREQ=DAILY",
            1_776_259_200_000,
            "none",
        )
        .unwrap();
        let diff = serde_json::json!({ "title": "do dishes" }).to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        let completions = chore::list_completions(&conn, chore_id, 10).unwrap();
        assert_eq!(completions.len(), 1);
    }

    #[test]
    fn approve_rejects_unknown_title() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({ "title": "No such chore" }).to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(matches!(err, ApplyError::StaleReference { entity, .. } if entity == "chore"));
    }
}
