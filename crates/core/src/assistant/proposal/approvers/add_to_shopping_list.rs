//! Approver for `add_to_shopping_list` proposals.

use chrono::Utc;
use rusqlite::{params, Transaction};

use crate::assistant::proposal::{AddShoppingListItem, AddToShoppingListArgs, Status};
use crate::assistant::{Applied, ApplyError};
use crate::shopping_list::dal;

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: AddToShoppingListArgs =
        serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
            field: "diff".into(),
            reason: e.to_string(),
        })?;
    let items = args.into_items();
    validate(&items)?;

    for item in &items {
        dal::insert_manual(tx, item.item.trim())
            .map_err(|e| ApplyError::Internal(format!("shopping list insert failed: {e}")))?;
    }
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: items.len(),
        items_failed: 0,
        errors: vec![],
    })
}

fn validate(items: &[AddShoppingListItem]) -> Result<(), ApplyError> {
    if items.is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "items".into(),
            reason: "at least one shopping list item is required".into(),
        });
    }
    if items.iter().any(|item| item.item.trim().is_empty()) {
        return Err(ApplyError::InvalidArg {
            field: "item".into(),
            reason: "shopping list item cannot be empty".into(),
        });
    }
    Ok(())
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
                kind: "add_to_shopping_list",
                rationale: "module-test",
                diff_json: diff,
                skill: "hearth",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_inserts_single_manual_item() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({ "item": "milk" }).to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.items_applied, 1);
        let item: String = conn
            .query_row(
                "SELECT ingredient_name FROM shopping_list_item WHERE source = 'manual'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(item, "milk");
    }

    #[test]
    fn approve_inserts_bundle() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!([{ "item": "milk" }, { "item": "eggs" }]).to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.items_applied, 2);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM shopping_list_item", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn approve_rejects_empty_item() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({ "item": " " }).to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(matches!(err, ApplyError::InvalidArg { field, .. } if field == "item"));
    }
}
