//! Approver for `add_recipe_quick` proposals.

use chrono::Utc;
use rusqlite::{params, Transaction};

use crate::assistant::proposal::{
    AddRecipeQuickArgs, AddRecipeQuickIngredient, AddRecipeQuickIngredientInput, Status,
};
use crate::assistant::{Applied, ApplyError};
use crate::recipe::{dal, ImportMethod, IngredientLine, RecipeDraft};

pub fn approve(tx: &Transaction, proposal_id: i64, diff: &str) -> Result<Applied, ApplyError> {
    let args: AddRecipeQuickArgs =
        serde_json::from_str(diff).map_err(|e| ApplyError::InvalidArg {
            field: "diff".into(),
            reason: e.to_string(),
        })?;
    validate(&args)?;

    let draft = RecipeDraft {
        title: args.title.trim().to_string(),
        servings: args.servings,
        prep_time_mins: args.prep_time_mins,
        cook_time_mins: args.cook_time_mins,
        instructions: steps_to_markdown(&args.steps),
        source_url: None,
        source_host: None,
        import_method: ImportMethod::Manual,
        ingredients: args
            .ingredients
            .iter()
            .map(ingredient_to_line)
            .collect::<Vec<_>>(),
        hero_attachment_uuid: None,
    };
    dal::insert_recipe(tx, &draft)
        .map_err(|e| ApplyError::Internal(format!("recipe insert failed: {e}")))?;
    mark_applied(tx, proposal_id)?;

    Ok(Applied {
        proposal_id,
        status: Status::Applied,
        items_applied: 1,
        items_failed: 0,
        errors: vec![],
    })
}

fn validate(args: &AddRecipeQuickArgs) -> Result<(), ApplyError> {
    if args.title.trim().is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "title".into(),
            reason: "recipe title cannot be empty".into(),
        });
    }
    if args.ingredients.is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "ingredients".into(),
            reason: "at least one ingredient is required".into(),
        });
    }
    if args.steps.is_empty() {
        return Err(ApplyError::InvalidArg {
            field: "steps".into(),
            reason: "at least one step is required".into(),
        });
    }
    if args.ingredients.iter().any(ingredient_is_empty) {
        return Err(ApplyError::InvalidArg {
            field: "ingredients".into(),
            reason: "ingredient name cannot be empty".into(),
        });
    }
    if args.steps.iter().any(|step| step.trim().is_empty()) {
        return Err(ApplyError::InvalidArg {
            field: "steps".into(),
            reason: "step cannot be empty".into(),
        });
    }
    Ok(())
}

fn ingredient_is_empty(input: &AddRecipeQuickIngredientInput) -> bool {
    match input {
        AddRecipeQuickIngredientInput::Text(text) => text.trim().is_empty(),
        AddRecipeQuickIngredientInput::Structured(ingredient) => {
            ingredient.ingredient_name.trim().is_empty()
        }
    }
}

fn ingredient_to_line(input: &AddRecipeQuickIngredientInput) -> IngredientLine {
    match input {
        AddRecipeQuickIngredientInput::Text(text) => IngredientLine {
            quantity_text: None,
            ingredient_name: text.trim().to_string(),
            note: None,
        },
        AddRecipeQuickIngredientInput::Structured(AddRecipeQuickIngredient {
            quantity_text,
            ingredient_name,
            note,
        }) => IngredientLine {
            quantity_text: quantity_text
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            ingredient_name: ingredient_name.trim().to_string(),
            note: note
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        },
    }
}

fn steps_to_markdown(steps: &[String]) -> String {
    steps
        .iter()
        .enumerate()
        .map(|(idx, step)| format!("{}. {}", idx + 1, step.trim()))
        .collect::<Vec<_>>()
        .join("\n")
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
                kind: "add_recipe_quick",
                rationale: "module-test",
                diff_json: diff,
                skill: "hearth",
            },
        )
        .unwrap()
    }

    #[test]
    fn approve_inserts_manual_recipe() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "title": "Miso pasta",
            "ingredients": [
                "pasta",
                { "quantity_text": "2 tbsp", "ingredient_name": "miso", "note": "white" }
            ],
            "steps": ["Boil pasta", "Stir through miso"],
            "servings": 2
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let applied = approve(&tx, pid, &diff).unwrap();
        tx.commit().unwrap();

        assert_eq!(applied.status, Status::Applied);
        let recipe_id: String = conn
            .query_row(
                "SELECT id FROM recipe WHERE title = 'Miso pasta'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let recipe = dal::get_recipe(&conn, &recipe_id).unwrap().unwrap();
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.instructions, "1. Boil pasta\n2. Stir through miso");
    }

    #[test]
    fn approve_rejects_empty_steps() {
        let (_d, mut conn) = fresh_conn();
        let diff = serde_json::json!({
            "title": "Miso pasta",
            "ingredients": ["pasta"],
            "steps": []
        })
        .to_string();
        let pid = make_proposal(&conn, &diff);

        let tx = conn.transaction().unwrap();
        let err = approve(&tx, pid, &diff).unwrap_err();
        assert!(matches!(err, ApplyError::InvalidArg { field, .. } if field == "steps"));
    }
}
