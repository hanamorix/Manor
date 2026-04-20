//! Maintenance event types (L4c).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Manual,
    Backfill,
}

impl EventSource {
    pub fn as_str(self) -> &'static str {
        match self {
            EventSource::Manual => "manual",
            EventSource::Backfill => "backfill",
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "manual" => Ok(EventSource::Manual),
            "backfill" => Ok(EventSource::Backfill),
            other => Err(anyhow::anyhow!("unknown EventSource: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEventDraft {
    pub asset_id: String,
    pub schedule_id: Option<String>,
    pub title: String,
    pub completed_date: String,
    pub cost_pence: Option<i64>,
    pub currency: String,
    pub notes: String,
    pub transaction_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEvent {
    pub id: String,
    pub asset_id: String,
    pub schedule_id: Option<String>,
    pub title: String,
    pub completed_date: String,
    pub cost_pence: Option<i64>,
    pub currency: String,
    pub notes: String,
    pub transaction_id: Option<i64>,
    pub source: EventSource,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventWithContext {
    pub event: MaintenanceEvent,
    pub schedule_task: Option<String>,
    pub schedule_deleted: bool,
    pub transaction_description: Option<String>,
    pub transaction_amount_pence: Option<i64>,
    pub transaction_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSpendTotal {
    pub asset_id: String,
    pub asset_name: String,
    pub asset_category: String,
    pub total_last_12m_pence: i64,
    pub total_lifetime_pence: i64,
    pub event_count_last_12m: i64,
    pub event_count_lifetime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySpendTotal {
    pub category: String,
    pub total_last_12m_pence: i64,
    pub total_lifetime_pence: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_source_round_trip() {
        assert_eq!(EventSource::parse("manual").unwrap(), EventSource::Manual);
        assert_eq!(
            EventSource::parse("backfill").unwrap(),
            EventSource::Backfill
        );
        assert_eq!(EventSource::Manual.as_str(), "manual");
        assert_eq!(EventSource::Backfill.as_str(), "backfill");
        assert!(EventSource::parse("other").is_err());
    }
}
