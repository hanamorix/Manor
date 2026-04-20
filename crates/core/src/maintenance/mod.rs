//! Maintenance schedules — types + pure computation + DAL.

pub mod dal;
pub mod due;
pub mod event;

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

#[cfg(test)]
mod migration_tests {
    use crate::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn v20_creates_maintenance_event_table() {
        let (_d, conn) = fresh_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='maintenance_event'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn v20_creates_tx_unique_partial_index() {
        let (_d, conn) = fresh_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_evt_tx_unique'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
