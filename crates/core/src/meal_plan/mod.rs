//! Meal plan — types + CRUD + staples. Pure data layer.

pub mod dal;
pub mod matcher;
pub mod staples;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealPlanEntry {
    pub id: String,
    pub entry_date: String,
    pub recipe_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StapleItem {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StapleDraft {
    pub name: String,
    pub aliases: Vec<String>,
}
