//! Approver for `add_recurring_payment` proposals.

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};

use crate::assistant::proposal::{AddRecurringPaymentArgs, Status};
use crate::assistant::{Applied, ApplyError};
use crate::ledger::{category, recurring};

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: AddRecurringPaymentArgs =
        serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
            field: "diff".into(),
            reason: e.to_string(),
        })?;
    validate(&args)?;

    let category_id = resolve_category(tx, &args)?;
    recurring::insert(
        tx,
        args.description.trim(),
        args.amount_pence,
        args.currency.trim(),
        category_id,
        args.day_of_month,
        args.note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty()),
    )
    .map_err(|e| ApplyError::Internal(format!("recurring payment insert failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn validate(args: &AddRecurringPaymentArgs) -> Result<(), ApplyError> {
    if args.description.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "description".into(),
            reason: "description cannot be empty".into(),
        });
    }
    if args.amount_pence <= 0 {
        return Err(ApplyError::InvalidArg {
            field: "amount_pence".into(),
            reason: "recurring payment amount must be positive".into(),
        });
    }
    if args.currency.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "currency".into(),
            reason: "currency cannot be empty".into(),
        });
    }
    if !(1..=28).contains(&args.day_of_month) {
        return Err(ApplyError::InvalidArg {
            field: "day_of_month".into(),
            reason: "day_of_month must be 1..=28".into(),
        });
    }
    Ok(())
}

fn resolve_category(
    tx: &Transaction,
    args: &AddRecurringPaymentArgs,
) -> Result<Option<i64>, ApplyError> {
    if let Some(id) = args.category_id {
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
            Ok(Some(id))
        } else {
            Err(ApplyError::StaleReference {
                entity: "category".into(),
                id: id.to_string(),
            })
        };
    }

    if let Some(name) = args
        .category_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
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

        return match matches.as_slice() {
            [id] => Ok(Some(*id)),
            [] => Err(ApplyError::StaleReference {
                entity: "category".into(),
                id: name.to_string(),
            }),
            _ => Err(ApplyError::Conflict(format!(
                "multiple categories match name '{name}'"
            ))),
        };
    }

    category::keyword_classify(tx, &args.description)
        .map_err(|e| ApplyError::Internal(format!("category classify failed: {e}")))
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
                kind: "add_recurring_payment",
                rationale: "module-test",
                diff_json: diff,
                skill: "ledger",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_inserts_recurring_payment_with_named_category() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "description": "Netflix",
            "amount_pence": "£12.99",
            "category_name": "Subscriptions",
            "day_of_month": 15,
            "note": "family plan"
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        let (amount, category_id, day): (i64, Option<i64>, i64) = conn
            .query_row(
                "SELECT amount_pence, category_id, day_of_month FROM recurring_payment WHERE description = 'Netflix'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(amount, 1299);
        assert_eq!(category_id, Some(5));
        assert_eq!(day, 15);
    }

    #[test]
    fn approve_rejects_invalid_day_of_month() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "description": "Netflix",
            "amount_pence": 1299,
            "day_of_month": 31
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(matches!(err, ApplyError::InvalidArg { field, .. } if field == "day_of_month"));
    }
}
