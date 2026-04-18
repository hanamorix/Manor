//! Recipe library — types + CRUD. Pure data layer; no network, no parsing.

pub mod dal;
pub mod import;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportMethod {
    Manual,
    JsonLd,
    Llm,
    LlmRemote,
}

impl ImportMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImportMethod::Manual => "manual",
            ImportMethod::JsonLd => "jsonld",
            ImportMethod::Llm => "llm",
            ImportMethod::LlmRemote => "llm_remote",
        }
    }

    pub fn from_db(s: Option<&str>) -> Self {
        match s {
            Some("jsonld") => Self::JsonLd,
            Some("llm") => Self::Llm,
            Some("llm_remote") => Self::LlmRemote,
            _ => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngredientLine {
    pub quantity_text: Option<String>,
    pub ingredient_name: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeDraft {
    pub title: String,
    pub servings: Option<i32>,
    pub prep_time_mins: Option<i32>,
    pub cook_time_mins: Option<i32>,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_host: Option<String>,
    pub import_method: ImportMethod,
    pub ingredients: Vec<IngredientLine>,
    /// UUID of the hero attachment row (attachment.uuid, TEXT). Set by the
    /// importer after staging; round-tripped on edits so it survives a save.
    pub hero_attachment_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub title: String,
    pub servings: Option<i32>,
    pub prep_time_mins: Option<i32>,
    pub cook_time_mins: Option<i32>,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_host: Option<String>,
    pub import_method: ImportMethod,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub ingredients: Vec<IngredientLine>,
    /// UUID of the hero attachment row. NULL when no hero image has been staged.
    pub hero_attachment_uuid: Option<String>,
}
