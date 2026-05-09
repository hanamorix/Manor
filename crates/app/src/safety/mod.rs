//! Safety module — trash, snapshot backup, panic button, launchd.

/// Private CLI flag used by the launchd job to run an encrypted snapshot backup
/// without opening the Tauri UI.
pub const SCHEDULED_BACKUP_FLAG: &str = "--manor-snapshot-backup";

pub mod launchd;
pub mod panic_commands;
pub mod snapshot_commands;
pub mod trash_commands;
