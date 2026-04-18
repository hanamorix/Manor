//! Shopping list — types + CRUD + regenerator. Pure data layer.

pub mod dal;
pub mod generator;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemSource {
    Generated,
    Manual,
}

impl ItemSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemSource::Generated => "generated",
            ItemSource::Manual => "manual",
        }
    }
    pub fn from_db(s: Option<&str>) -> Self {
        match s {
            Some("generated") => Self::Generated,
            _ => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingListItem {
    pub id: String,
    pub ingredient_name: String,
    pub quantity_text: Option<String>,
    pub note: Option<String>,
    pub recipe_id: Option<String>,
    pub recipe_title: Option<String>,
    pub source: ItemSource,
    pub position: i64,
    pub ticked: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneratedReport {
    pub items_added: usize,
    pub items_skipped_staple: usize,
    pub ghost_recipes_skipped: usize,
}
