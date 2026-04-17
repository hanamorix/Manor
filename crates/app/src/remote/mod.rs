//! Remote LLM support — provider abstraction, keychain, orchestrator.
//! See `docs/superpowers/specs/2026-04-17-remote-llm-design.md`.

pub mod claude;
pub mod provider;

pub const PROVIDER_CLAUDE: &str = "claude";
pub const DEFAULT_MODEL_CLAUDE: &str = "claude-sonnet-4-6";
pub const DEFAULT_BUDGET_PENCE: i64 = 1000; // £10
pub const WARN_THRESHOLD_NUM: i64 = 75; // 75% of cap triggers warning
pub const REMOTE_ENABLED_FOR_REVIEW_KEY: &str = "ai.remote.enabled_for_review";

pub fn budget_setting_key(provider: &str) -> String {
    format!("budget.{provider}_monthly_pence")
}
