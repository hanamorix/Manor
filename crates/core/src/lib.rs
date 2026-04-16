//! Manor core library.

pub mod assistant;
pub mod ledger;
pub mod setting;

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
