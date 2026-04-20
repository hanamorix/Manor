//! PDF manual extraction (L4e).

pub mod llm;
pub mod text;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("PDF too large to extract (over {0} MB)")]
    TooLarge(u64),
    #[error("PDF appears to be an image scan — text extraction isn't possible")]
    ImageOnly,
    #[error("couldn't read PDF file: {0}")]
    ReadFailed(String),
    #[error("couldn't parse PDF: {0}")]
    ParseFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedSchedule {
    pub task: String,
    pub interval_months: i32,
    pub notes: String,
    pub rationale: String,
}
