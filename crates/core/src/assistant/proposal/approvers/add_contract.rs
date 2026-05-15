//! Approver for `add_contract` proposals.

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};

use crate::assistant::proposal::{AddContractArgs, Status};
use crate::assistant::{Applied, ApplyError};
use crate::ledger::contract::{self, NewContract};

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: AddContractArgs = serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
        field: "diff".into(),
        reason: e.to_string(),
    })?;
    validate(&args)?;
    validate_recurring_payment(tx, args.recurring_payment_id)?;

    contract::insert(
        tx,
        NewContract {
            provider: args.provider.trim(),
            kind: args.kind.trim(),
            description: args
                .description
                .as_deref()
                .map(str::trim)
                .filter(|description| !description.is_empty()),
            monthly_cost_pence: args.monthly_cost_pence,
            term_start: args.term_start,
            term_end: args.term_end,
            exit_fee_pence: args.exit_fee_pence,
            renewal_alert_days: args.renewal_alert_days,
            recurring_payment_id: args.recurring_payment_id,
            note: args
                .note
                .as_deref()
                .map(str::trim)
                .filter(|note| !note.is_empty()),
        },
    )
    .map_err(|e| ApplyError::Internal(format!("contract insert failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn validate(args: &AddContractArgs) -> Result<(), ApplyError> {
    if args.provider.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "provider".into(),
            reason: "provider cannot be empty".into(),
        });
    }
    if args.kind.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "kind".into(),
            reason: "kind cannot be empty".into(),
        });
    }
    if args.monthly_cost_pence <= 0 {
        return Err(ApplyError::InvalidArg {
            field: "monthly_cost_pence".into(),
            reason: "monthly cost must be positive".into(),
        });
    }
    if args.term_end <= args.term_start {
        return Err(ApplyError::InvalidArg {
            field: "term_end".into(),
            reason: "term_end must be after term_start".into(),
        });
    }
    if matches!(args.exit_fee_pence, Some(exit_fee) if exit_fee < 0) {
        return Err(ApplyError::InvalidArg {
            field: "exit_fee_pence".into(),
            reason: "exit fee cannot be negative".into(),
        });
    }
    if args.renewal_alert_days < 0 {
        return Err(ApplyError::InvalidArg {
            field: "renewal_alert_days".into(),
            reason: "renewal alert days cannot be negative".into(),
        });
    }
    Ok(())
}

fn validate_recurring_payment(
    tx: &Transaction,
    recurring_payment_id: Option<i64>,
) -> Result<(), ApplyError> {
    let Some(id) = recurring_payment_id else {
        return Ok(());
    };
    let exists: bool = tx
        .query_row(
            "SELECT 1 FROM recurring_payment WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| ApplyError::Internal(format!("recurring payment lookup failed: {e}")))?
        .unwrap_or(false);
    if exists {
        Ok(())
    } else {
        Err(ApplyError::StaleReference {
            entity: "recurring_payment".into(),
            id: id.to_string(),
        })
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
    use crate::ledger::recurring;
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
                kind: "add_contract",
                rationale: "module-test",
                diff_json: diff,
                skill: "ledger",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_inserts_contract_with_recurring_payment() {
        let (_d, mut conn) = fresh_conn();
        let recurring =
            recurring::insert(&conn, "Broadband", 3000, "GBP", Some(4), 4, None).unwrap();
        let diff = serde_json::json!({
            "provider": "Zen Internet",
            "kind": "broadband",
            "description": "Home broadband",
            "monthly_cost_pence": "£30",
            "term_start": 1_767_225_600i64,
            "term_end": 1_798_761_600i64,
            "exit_fee_pence": "£50",
            "renewal_alert_days": 45,
            "recurring_payment_id": recurring.id
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        let (monthly_cost, exit_fee, recurring_id): (i64, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT monthly_cost_pence, exit_fee_pence, recurring_payment_id FROM contract WHERE provider = 'Zen Internet'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(monthly_cost, 3000);
        assert_eq!(exit_fee, Some(5000));
        assert_eq!(recurring_id, Some(recurring.id));
    }

    #[test]
    fn approve_rejects_stale_recurring_payment() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "provider": "Zen Internet",
            "kind": "broadband",
            "monthly_cost_pence": 3000,
            "term_start": 1_767_225_600i64,
            "term_end": 1_798_761_600i64,
            "recurring_payment_id": 9999
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(
            matches!(err, ApplyError::StaleReference { entity, .. } if entity == "recurring_payment")
        );
    }
}
