//! Approver for `add_transaction` proposals.

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};

use crate::assistant::proposal::{AddLedgerTransactionArgs, Status};
use crate::assistant::{Applied, ApplyError};
use crate::ledger::{category, transaction};

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: AddLedgerTransactionArgs =
        serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
            field: "diff".into(),
            reason: e.to_string(),
        })?;
    validate(&args)?;

    let category_id = resolve_category(tx, &args)?;
    let date = args.date.unwrap_or_else(|| Utc::now().timestamp());

    transaction::insert(
        tx,
        args.amount_pence,
        args.currency.trim(),
        args.description.trim(),
        args.merchant
            .as_deref()
            .map(str::trim)
            .filter(|merchant| !merchant.is_empty()),
        category_id,
        date,
        args.note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty()),
    )
    .map_err(|e| ApplyError::Internal(format!("transaction insert failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn validate(args: &AddLedgerTransactionArgs) -> Result<(), ApplyError> {
    if args.amount_pence == 0 {
        return Err(ApplyError::InvalidArg {
            field: "amount_pence".into(),
            reason: "amount cannot be zero".into(),
        });
    }
    if args.currency.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "currency".into(),
            reason: "currency cannot be empty".into(),
        });
    }
    if args.description.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "description".into(),
            reason: "description cannot be empty".into(),
        });
    }
    Ok(())
}

fn resolve_category(
    tx: &Transaction,
    args: &AddLedgerTransactionArgs,
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

    category::keyword_classify(
        tx,
        &format!(
            "{} {}",
            args.description,
            args.merchant.as_deref().unwrap_or("")
        ),
    )
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
                kind: "add_transaction",
                rationale: "module-test",
                diff_json: diff,
                skill: "ledger",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_inserts_manual_transaction_with_named_category() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "amount_pence": "-£12.40",
            "description": "Tesco Express",
            "merchant": "Tesco",
            "category_name": "Groceries",
            "date": 1_777_132_800i64
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        let (amount, category_id): (i64, Option<i64>) = conn
            .query_row(
                "SELECT amount_pence, category_id FROM ledger_transaction WHERE description = 'Tesco Express'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(amount, -1240);
        assert_eq!(category_id, Some(1));
    }

    #[test]
    fn approve_rejects_missing_named_category() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "amount_pence": -1240,
            "description": "Tesco Express",
            "category_name": "Nope",
            "date": 1_777_132_800i64
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(matches!(err, ApplyError::StaleReference { entity, .. } if entity == "category"));
    }
}
