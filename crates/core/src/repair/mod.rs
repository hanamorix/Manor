//! Repair-note types (L4d).

pub mod dal;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTier {
    Ollama,
    Claude,
}

impl LlmTier {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmTier::Ollama => "ollama",
            LlmTier::Claude => "claude",
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "ollama" => Ok(LlmTier::Ollama),
            "claude" => Ok(LlmTier::Claude),
            other => Err(anyhow::anyhow!("unknown LlmTier: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairSource {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairNoteDraft {
    pub asset_id: String,
    pub symptom: String,
    pub body_md: String,
    pub sources: Vec<RepairSource>,
    pub video_sources: Option<Vec<RepairSource>>,
    pub tier: LlmTier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairNote {
    pub id: String,
    pub asset_id: String,
    pub symptom: String,
    pub body_md: String,
    pub sources: Vec<RepairSource>,
    pub video_sources: Option<Vec<RepairSource>>,
    pub tier: LlmTier,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
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
    fn v21_creates_repair_note_table() {
        let (_d, conn) = fresh_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='repair_note'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn v21_creates_repair_indexes() {
        let (_d, conn) = fresh_conn();
        for name in &[
            "idx_repair_asset",
            "idx_repair_created",
            "idx_repair_deleted",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "index {} missing", name);
        }
    }

    #[test]
    fn llm_tier_round_trip() {
        use super::LlmTier;
        assert_eq!(LlmTier::parse("ollama").unwrap(), LlmTier::Ollama);
        assert_eq!(LlmTier::parse("claude").unwrap(), LlmTier::Claude);
        assert_eq!(LlmTier::Ollama.as_str(), "ollama");
        assert_eq!(LlmTier::Claude.as_str(), "claude");
        assert!(LlmTier::parse("other").is_err());
    }
}
