//! Meal ideas ranker — scores library recipes by days_since_last_cooked.
//! Pure function; no randomness (caller shuffles ties).

use crate::recipe::{dal::ListFilter, Recipe};
use anyhow::Result;
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const NEVER_COOKED_SCORE: i64 = 9999;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredRecipe {
    pub recipe: Recipe,
    pub score: i64,
}

/// Load all non-trashed recipes, compute days-since-last-cooked per recipe,
/// sort descending by score (never-cooked = 9999). Deterministic — no shuffle here.
pub fn library_ranked(conn: &Connection) -> Result<Vec<ScoredRecipe>> {
    let recipes = crate::recipe::dal::list_recipes(conn, &ListFilter::default())?;
    let today = chrono::Local::now().date_naive();

    let mut scored: Vec<ScoredRecipe> = recipes.into_iter().map(|recipe| {
        let score = days_since_last_cooked(conn, &recipe.id, today).unwrap_or(NEVER_COOKED_SCORE);
        ScoredRecipe { recipe, score }
    }).collect();

    scored.sort_by(|a, b| b.score.cmp(&a.score));
    Ok(scored)
}

fn days_since_last_cooked(conn: &Connection, recipe_id: &str, today: NaiveDate) -> Option<i64> {
    let max_date: Option<String> = conn.query_row(
        "SELECT MAX(entry_date) FROM meal_plan_entry WHERE recipe_id = ?1",
        rusqlite::params![recipe_id],
        |r| r.get(0),
    ).ok().flatten();

    let s = max_date?;
    let last = NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()?;
    let diff = today.signed_duration_since(last).num_days();
    Some(diff.max(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use crate::recipe::{ImportMethod, RecipeDraft};
    use tempfile::tempdir;

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn insert_recipe(conn: &Connection, title: &str) -> String {
        let draft = RecipeDraft {
            title: title.into(),
            servings: None, prep_time_mins: None, cook_time_mins: None,
            instructions: "".into(),
            source_url: None, source_host: None,
            import_method: ImportMethod::Manual,
            hero_attachment_uuid: None,
            ingredients: vec![],
        };
        crate::recipe::dal::insert_recipe(conn, &draft).unwrap()
    }

    #[test]
    fn empty_library_returns_empty_vec() {
        let (_d, conn) = fresh();
        let ranked = library_ranked(&conn).unwrap();
        assert!(ranked.is_empty());
    }

    #[test]
    fn never_cooked_scores_sentinel() {
        let (_d, conn) = fresh();
        let id = insert_recipe(&conn, "Miso");
        let ranked = library_ranked(&conn).unwrap();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].recipe.id, id);
        assert_eq!(ranked[0].score, NEVER_COOKED_SCORE);
    }

    #[test]
    fn sorted_descending_by_days_since() {
        let (_d, conn) = fresh();
        let a = insert_recipe(&conn, "A");
        let b = insert_recipe(&conn, "B");
        let c = insert_recipe(&conn, "C");

        let today = chrono::Local::now().date_naive();
        let a_date = (today - chrono::Duration::days(2)).format("%Y-%m-%d").to_string();
        let b_date = (today - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
        let c_date = (today - chrono::Duration::days(10)).format("%Y-%m-%d").to_string();
        crate::meal_plan::dal::set_entry(&conn, &a_date, &a).unwrap();
        crate::meal_plan::dal::set_entry(&conn, &b_date, &b).unwrap();
        crate::meal_plan::dal::set_entry(&conn, &c_date, &c).unwrap();

        let ranked = library_ranked(&conn).unwrap();
        let ordered_ids: Vec<_> = ranked.iter().map(|s| s.recipe.id.as_str()).collect();
        assert_eq!(ordered_ids, vec![b.as_str(), c.as_str(), a.as_str()]);
        assert_eq!(ranked[0].score, 30);
        assert_eq!(ranked[1].score, 10);
        assert_eq!(ranked[2].score, 2);
    }

    #[test]
    fn trashed_recipe_excluded() {
        let (_d, conn) = fresh();
        let alive = insert_recipe(&conn, "Alive");
        let dead = insert_recipe(&conn, "Gone");
        crate::recipe::dal::soft_delete_recipe(&conn, &dead).unwrap();

        let ranked = library_ranked(&conn).unwrap();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].recipe.id, alive);
    }

    #[test]
    fn never_cooked_ranks_above_any_cooked() {
        let (_d, conn) = fresh();
        let cooked = insert_recipe(&conn, "Cooked");
        let fresh_r = insert_recipe(&conn, "Fresh");

        let today = chrono::Local::now().date_naive();
        let d = (today - chrono::Duration::days(100)).format("%Y-%m-%d").to_string();
        crate::meal_plan::dal::set_entry(&conn, &d, &cooked).unwrap();

        let ranked = library_ranked(&conn).unwrap();
        // Fresh (never cooked, score 9999) should rank first.
        assert_eq!(ranked[0].recipe.id, fresh_r);
        assert_eq!(ranked[0].score, NEVER_COOKED_SCORE);
        assert_eq!(ranked[1].recipe.id, cooked);
        assert_eq!(ranked[1].score, 100);
    }
}
