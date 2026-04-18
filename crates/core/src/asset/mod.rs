//! Asset registry — types + CRUD. Pure data layer.

pub mod dal;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetCategory {
    Appliance,
    Vehicle,
    Fixture,
    Other,
}

impl AssetCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssetCategory::Appliance => "appliance",
            AssetCategory::Vehicle => "vehicle",
            AssetCategory::Fixture => "fixture",
            AssetCategory::Other => "other",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "appliance" => Self::Appliance,
            "vehicle" => Self::Vehicle,
            "fixture" => Self::Fixture,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDraft {
    pub name: String,
    pub category: AssetCategory,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub purchase_date: Option<String>,
    pub notes: String,
    pub hero_attachment_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub category: AssetCategory,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub purchase_date: Option<String>,
    pub notes: String,
    pub hero_attachment_uuid: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}
