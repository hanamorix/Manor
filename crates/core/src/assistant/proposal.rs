//! Proposals — central AI-action artefacts.
//!
//! Phase 2 scaffolds the table + types but no feature produces proposals yet.
//! Later phases (Rhythm, Ledger, Hearth, Bones) INSERT rows when their skills
//! need a reviewable diff.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Approved,
    Rejected,
    Applied,
    PartiallyApplied,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Approved => "approved",
            Status::Rejected => "rejected",
            Status::Applied => "applied",
            Status::PartiallyApplied => "partially_applied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProposal<'a> {
    pub kind: &'a str,
    pub rationale: &'a str,
    pub diff_json: &'a str,
    pub skill: &'a str,
}

/// Insert a new proposal. Returns the new row id.
pub fn insert(conn: &Connection, new: NewProposal<'_>) -> Result<i64> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO proposal (kind, rationale, diff, status, proposed_at, skill)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            new.kind,
            new.rationale,
            new.diff_json,
            Status::Pending.as_str(),
            now,
            new.skill,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    #[test]
    fn insert_returns_new_row_id() {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();

        let id = insert(
            &conn,
            NewProposal {
                kind: "week_plan",
                rationale: "Automated test proposal",
                diff_json: "{\"ops\":[]}",
                skill: "calendar",
            },
        )
        .unwrap();
        assert!(id > 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proposal WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }
}
