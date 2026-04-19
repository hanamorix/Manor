//! Maintenance schedules — types + pure computation + DAL.

pub mod dal;
pub mod due;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceScheduleDraft {
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub last_done_date: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSchedule {
    pub id: String,
    pub asset_id: String,
    pub task: String,
    pub interval_months: i32,
    pub last_done_date: Option<String>,
    pub next_due_date: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DueBand {
    Overdue,
    DueThisWeek,
    Upcoming,
    Far,
}
