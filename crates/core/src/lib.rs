//! Manor core library.

pub mod assistant;
pub mod attachment;
pub mod embedding;
pub mod household;
pub mod ledger;
pub mod meal_plan;
pub mod note;
pub mod person;
pub mod recipe;
pub mod redact;
pub mod remote_call_log;
pub mod setting;
pub mod shopping_list;
pub mod snapshot;
pub mod tag;
pub mod trash;

/// Returns the crate version string, used by the shell for the About screen.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(
            !version().is_empty(),
            "version should return a non-empty string"
        );
    }

    #[test]
    fn version_matches_cargo_pkg() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
