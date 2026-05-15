//! Approver for `set_budget` proposals.

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};

use crate::assistant::proposal::{SetBudgetArgs, Status};
use crate::assistant::{Applied, ApplyError};
use crate::ledger::budget;

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: SetBudgetArgs = serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
        field: "diff".into(),
        reason: e.to_string(),
    })?;
    if args.amount_pence <= 0 {
        return Err(ApplyError::InvalidArg {
            field: "amount_pence".into(),
            reason: "budget amount must be positive".into(),
        });
    }

    let category_id = resolve_category(tx, args.category_id, args.category_name.as_deref())?;
    budget::upsert(tx, category_id, args.amount_pence)
        .map_err(|e| ApplyError::Internal(format!("budget upsert failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn resolve_category(
    tx: &Transaction,
    category_id: Option<i64>,
    category_name: Option<&str>,
) -> Result<i64, ApplyError> {
    if let Some(id) = category_id {
        let exists: bool = tx
            .query_row(
                "SELECT 1 FROM category WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| ApplyError::Internal(format!("category lookup failed: {e}")))?
            .unwrap_or(false);
        return if exists {
            Ok(id)
        } else {
            Err(ApplyError::StaleReference {
                entity: "category".into(),
                id: id.to_string(),
            })
        };
    }

    let name = category_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ApplyError::InvalidArg {
            field: "category_name".into(),
            reason: "category_id or category_name is required".into(),
        })?;

    let matches: Vec<i64> = {
        let mut stmt = tx
            .prepare(
                "SELECT id FROM category
                 WHERE deleted_at IS NULL AND lower(name) = lower(?1)
                 ORDER BY id",
            )
            .map_err(|e| ApplyError::Internal(format!("category lookup failed: {e}")))?;
        let rows = stmt
            .query_map([name], |row| row.get(0))
            .map_err(|e| ApplyError::Internal(format!("category lookup failed: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApplyError::Internal(format!("category lookup failed: {e}")))?;
        rows
    };

    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(ApplyError::StaleReference {
            entity: "category".into(),
            id: name.to_string(),
        }),
        _ => Err(ApplyError::Conflict(format!(
            "multiple categories match name '{name}'"
        ))),
    }
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
                kind: "set_budget",
                rationale: "module-test",
                diff_json: diff,
                skill: "ledger",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_sets_budget_by_category_name() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "category_name": "Groceries",
            "amount_pence": "£400"
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        let amount: i64 = conn
            .query_row(
                "SELECT amount_pence FROM budget WHERE category_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(amount, 40000);
    }

    #[test]
    fn approve_rejects_non_positive_budget() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "category_name": "Groceries",
            "amount_pence": 0
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(matches!(err, ApplyError::InvalidArg { field, .. } if field == "amount_pence"));
    }
}
