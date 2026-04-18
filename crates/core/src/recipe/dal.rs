use super::{ImportMethod, IngredientLine, Recipe, RecipeDraft};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn insert_recipe(conn: &Connection, draft: &RecipeDraft) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO recipe (id, title, servings, prep_time_mins, cook_time_mins,
            instructions, source_url, source_host, import_method, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            draft.title,
            draft.servings,
            draft.prep_time_mins,
            draft.cook_time_mins,
            draft.instructions,
            draft.source_url,
            draft.source_host,
            draft.import_method.as_str(),
            now,
            now,
        ],
    )?;
    for (pos, ing) in draft.ingredients.iter().enumerate() {
        conn.execute(
            "INSERT INTO recipe_ingredient (id, recipe_id, position, quantity_text, ingredient_name, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                id,
                pos as i64,
                ing.quantity_text,
                ing.ingredient_name,
                ing.note,
            ],
        )?;
    }
    Ok(id)
}

pub fn get_recipe(conn: &Connection, id: &str) -> Result<Option<Recipe>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, servings, prep_time_mins, cook_time_mins, instructions,
                source_url, source_host, import_method, created_at, updated_at, deleted_at
         FROM recipe WHERE id = ?1",
    )?;
    let recipe = stmt
        .query_row(params![id], |row| {
            let import_method_str: Option<String> = row.get(8)?;
            Ok(Recipe {
                id: row.get(0)?,
                title: row.get(1)?,
                servings: row.get(2)?,
                prep_time_mins: row.get(3)?,
                cook_time_mins: row.get(4)?,
                instructions: row.get(5)?,
                source_url: row.get(6)?,
                source_host: row.get(7)?,
                import_method: ImportMethod::from_db(import_method_str.as_deref()),
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                deleted_at: row.get(11)?,
                ingredients: Vec::new(),
            })
        })
        .optional()?;

    let Some(mut recipe) = recipe else {
        return Ok(None);
    };

    let mut s2 = conn.prepare(
        "SELECT quantity_text, ingredient_name, note
         FROM recipe_ingredient WHERE recipe_id = ?1 ORDER BY position ASC",
    )?;
    let rows = s2.query_map(params![id], |r| {
        Ok(IngredientLine {
            quantity_text: r.get(0)?,
            ingredient_name: r.get(1)?,
            note: r.get(2)?,
        })
    })?;
    for row in rows {
        recipe.ingredients.push(row?);
    }
    Ok(Some(recipe))
}

#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub search: Option<String>,
    pub tag_ids: Vec<String>,
    pub include_trashed: bool,
}

pub fn list_recipes(conn: &Connection, filter: &ListFilter) -> Result<Vec<Recipe>> {
    let mut sql = String::from(
        "SELECT id, title, servings, prep_time_mins, cook_time_mins, instructions,
                source_url, source_host, import_method, created_at, updated_at, deleted_at
         FROM recipe WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if !filter.include_trashed {
        sql.push_str(" AND deleted_at IS NULL");
    }
    if let Some(q) = filter.search.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND title LIKE ?");
        params.push(Box::new(format!("%{}%", q)));
    }
    sql.push_str(" ORDER BY created_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(params_ref.as_slice(), |row| {
        let import_method_str: Option<String> = row.get(8)?;
        Ok(Recipe {
            id: row.get(0)?,
            title: row.get(1)?,
            servings: row.get(2)?,
            prep_time_mins: row.get(3)?,
            cook_time_mins: row.get(4)?,
            instructions: row.get(5)?,
            source_url: row.get(6)?,
            source_host: row.get(7)?,
            import_method: ImportMethod::from_db(import_method_str.as_deref()),
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
            deleted_at: row.get(11)?,
            ingredients: Vec::new(),
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update_recipe(conn: &Connection, id: &str, draft: &RecipeDraft) -> Result<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE recipe SET title=?1, servings=?2, prep_time_mins=?3, cook_time_mins=?4,
            instructions=?5, source_url=?6, source_host=?7, import_method=?8, updated_at=?9
         WHERE id=?10",
        params![
            draft.title,
            draft.servings,
            draft.prep_time_mins,
            draft.cook_time_mins,
            draft.instructions,
            draft.source_url,
            draft.source_host,
            draft.import_method.as_str(),
            now,
            id,
        ],
    )?;
    conn.execute(
        "DELETE FROM recipe_ingredient WHERE recipe_id=?1",
        params![id],
    )?;
    for (pos, ing) in draft.ingredients.iter().enumerate() {
        conn.execute(
            "INSERT INTO recipe_ingredient (id, recipe_id, position, quantity_text, ingredient_name, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                id,
                pos as i64,
                ing.quantity_text,
                ing.ingredient_name,
                ing.note,
            ],
        )?;
    }
    Ok(())
}

pub fn soft_delete_recipe(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE recipe SET deleted_at=?1 WHERE id=?2",
        params![now_ms(), id],
    )?;
    Ok(())
}

pub fn restore_recipe(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE recipe SET deleted_at=NULL WHERE id=?1",
        params![id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_db() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn insert_and_get_recipe_roundtrips() {
        let (_d, conn) = fresh_db();
        let draft = RecipeDraft {
            title: "Miso aubergine".into(),
            servings: Some(4),
            prep_time_mins: Some(15),
            cook_time_mins: Some(30),
            instructions: "1. Preheat oven...".into(),
            source_url: None,
            source_host: None,
            import_method: ImportMethod::Manual,
            ingredients: vec![IngredientLine {
                quantity_text: Some("2".into()),
                ingredient_name: "aubergines".into(),
                note: None,
            }],
        };
        let id = insert_recipe(&conn, &draft).unwrap();
        let got = get_recipe(&conn, &id).unwrap().expect("recipe exists");
        assert_eq!(got.title, "Miso aubergine");
        assert_eq!(got.ingredients.len(), 1);
        assert_eq!(got.ingredients[0].ingredient_name, "aubergines");
    }

    #[test]
    fn list_excludes_trashed_by_default() {
        let (_d, conn) = fresh_db();
        let a = insert_recipe(&conn, &simple_draft("A")).unwrap();
        let _b = insert_recipe(&conn, &simple_draft("B")).unwrap();
        soft_delete_recipe(&conn, &a).unwrap();
        let list = list_recipes(&conn, &ListFilter::default()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "B");
    }

    #[test]
    fn update_bumps_updated_at_and_replaces_ingredients() {
        let (_d, conn) = fresh_db();
        let id = insert_recipe(&conn, &simple_draft("Original")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut draft = simple_draft("Updated");
        draft.ingredients = vec![IngredientLine {
            quantity_text: Some("5".into()),
            ingredient_name: "garlic".into(),
            note: None,
        }];
        update_recipe(&conn, &id, &draft).unwrap();
        let r = get_recipe(&conn, &id).unwrap().unwrap();
        assert_eq!(r.title, "Updated");
        assert_eq!(r.ingredients.len(), 1);
        assert_eq!(r.ingredients[0].ingredient_name, "garlic");
        assert!(r.updated_at > r.created_at);
    }

    #[test]
    fn restore_clears_deleted_at() {
        let (_d, conn) = fresh_db();
        let id = insert_recipe(&conn, &simple_draft("X")).unwrap();
        soft_delete_recipe(&conn, &id).unwrap();
        assert!(get_recipe(&conn, &id).unwrap().unwrap().deleted_at.is_some());
        restore_recipe(&conn, &id).unwrap();
        assert!(get_recipe(&conn, &id).unwrap().unwrap().deleted_at.is_none());
    }

    fn simple_draft(title: &str) -> RecipeDraft {
        RecipeDraft {
            title: title.into(),
            servings: None,
            prep_time_mins: None,
            cook_time_mins: None,
            instructions: "Cook it.".into(),
            source_url: None,
            source_host: None,
            import_method: ImportMethod::Manual,
            ingredients: vec![],
        }
    }
}
