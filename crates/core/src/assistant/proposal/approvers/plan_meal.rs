//! Approver for `plan_meal` proposals.

use chrono::{NaiveDate, Utc};
use rusqlite::{params, OptionalExtension, Transaction};

use crate::assistant::proposal::{PlanMealArgs, Status};
use crate::assistant::{Applied, ApplyError};
use crate::meal_plan::dal;

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: PlanMealArgs = serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
        field: "diff".into(),
        reason: e.to_string(),
    })?;
    let date = validate_date(&args.date_iso)?;
    let recipe_id = resolve_recipe(tx, &args)?;

    dal::set_entry(tx, &date, &recipe_id)
        .map_err(|e| ApplyError::Internal(format!("meal plan set failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn validate_date(raw: &str) -> Result<String, ApplyError> {
    let trimmed = raw.trim();
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|e| ApplyError::InvalidArg {
        field: "date_iso".into(),
        reason: format!("date must be YYYY-MM-DD: {e}"),
    })?;
    Ok(trimmed.to_string())
}

fn resolve_recipe(tx: &Transaction, args: &PlanMealArgs) -> Result<String, ApplyError> {
    if let Some(id) = args
        .recipe_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        return resolve_recipe_id(tx, id);
    }
    if let Some(name) = args
        .recipe_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return resolve_recipe_name(tx, name);
    }
    if let Some(value) = args
        .recipe_id_or_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Ok(id) = resolve_recipe_id(tx, value) {
            return Ok(id);
        }
        return resolve_recipe_name(tx, value);
    }
    Err(ApplyError::InvalidArg {
        field: "recipe".into(),
        reason: "recipe_id, recipe_name, or recipe_id_or_name is required".into(),
    })
}

fn resolve_recipe_id(tx: &Transaction, id: &str) -> Result<String, ApplyError> {
    let exists: bool = tx
        .query_row(
            "SELECT 1 FROM recipe WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| ApplyError::Internal(format!("recipe lookup failed: {e}")))?
        .unwrap_or(false);
    if exists {
        Ok(id.to_string())
    } else {
        Err(ApplyError::StaleReference {
            entity: "recipe".into(),
            id: id.to_string(),
        })
    }
}

fn resolve_recipe_name(tx: &Transaction, name: &str) -> Result<String, ApplyError> {
    let matches: Vec<String> = {
        let mut stmt = tx
            .prepare(
                "SELECT id FROM recipe
                 WHERE deleted_at IS NULL AND lower(title) = lower(?1)
                 ORDER BY id",
            )
            .map_err(|e| ApplyError::Internal(format!("recipe lookup failed: {e}")))?;
        let rows = stmt
            .query_map([name], |row| row.get(0))
            .map_err(|e| ApplyError::Internal(format!("recipe lookup failed: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApplyError::Internal(format!("recipe lookup failed: {e}")))?;
        rows
    };

    match matches.as_slice() {
        [id] => Ok(id.clone()),
        [] => Err(ApplyError::StaleReference {
            entity: "recipe".into(),
            id: name.to_string(),
        }),
        _ => Err(ApplyError::Conflict(format!(
            "multiple recipes match title '{name}'"
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
    use crate::recipe::{dal as recipe_dal, ImportMethod, RecipeDraft};
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn insert_recipe(conn: &Connection, title: &str) -> String {
        recipe_dal::insert_recipe(
            conn,
            &RecipeDraft {
                title: title.into(),
                servings: None,
                prep_time_mins: None,
                cook_time_mins: None,
                instructions: "1. Cook".into(),
                source_url: None,
                source_host: None,
                import_method: ImportMethod::Manual,
                ingredients: vec![],
                hero_attachment_uuid: None,
            },
        )
        .unwrap()
    }

    fn make_proposal(conn: &Connection, diff: &str) -> i64 {
        insert(
            conn,
            NewProposal {
                kind: "plan_meal",
                rationale: "module-test",
                diff_json: diff,
                skill: "hearth",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_sets_meal_plan_by_recipe_name() {
        let (_d, mut conn) = fresh_conn();
        let recipe_id = insert_recipe(&conn, "Miso pasta");
        let diff = serde_json::json!({
            "date_iso": "2026-05-18",
            "recipe_name": "Miso pasta"
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        let planned: String = conn
            .query_row(
                "SELECT recipe_id FROM meal_plan_entry WHERE entry_date = '2026-05-18'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(planned, recipe_id);
    }

    #[test]
    fn approve_rejects_unknown_recipe() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "date_iso": "2026-05-18",
            "recipe_name": "Missing recipe"
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(matches!(err, ApplyError::StaleReference { entity, .. } if entity == "recipe"));
    }
}
