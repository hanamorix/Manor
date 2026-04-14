//! Tauri command glue for Life Assistant.
//!
//! Keeps tauri-specific types out of `life-core`. The Tauri shell calls
//! `register` to install all IPC commands defined in this crate.

use tauri::{Builder, Wry};

/// Registers every Tauri command this crate exposes.
///
/// Currently a pass-through; commands are added in later tasks.
pub fn register(builder: Builder<Wry>) -> Builder<Wry> {
    builder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_returns_builder() {
        // Smoke: function is callable without panicking.
        let _builder = register(Builder::default());
    }
}
