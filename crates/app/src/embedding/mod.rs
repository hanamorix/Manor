//! Embeddings — Ollama-backed semantic index + Tauri commands.

pub mod commands;
pub mod job;

pub const EMBED_MODEL_DEFAULT: &str = "nomic-embed-text";
pub const EMBED_MODEL_SETTING_KEY: &str = "embeddings.model";
pub const EMBED_BATCH_SIZE: usize = 50;
