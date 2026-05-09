//! launchd plist installer for weekly snapshot scheduling (macOS only).
//!
//! Target plist: `~/Library/LaunchAgents/com.hanamorix.manor.snapshot.plist`
//! The plist invokes a shell command that the app writes into the user's data dir.
//! This keeps the scheduled job out-of-process so it works even when the Manor app is closed.

use anyhow::{anyhow, Context, Result};
use plist::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LABEL: &str = "com.hanamorix.manor.snapshot";

pub fn plist_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME unset")?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

/// Weekday: 0 (Sun) – 6 (Sat). Hour 0–23. Minute 0–59.
pub fn install(
    program_path: &Path,
    arg_out_dir: &Path,
    weekday: u8,
    hour: u8,
    minute: u8,
) -> Result<()> {
    let plist = build_plist(program_path, arg_out_dir, weekday, hour, minute)?;

    let path = plist_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    plist::to_file_xml(&path, &plist)?;

    // Load it. launchctl load is idempotent-ish; unload any existing copy first.
    let _ = Command::new("launchctl").arg("unload").arg(&path).output();
    let out = Command::new("launchctl")
        .arg("load")
        .arg(&path)
        .output()
        .context("failed to exec launchctl")?;
    if !out.status.success() {
        return Err(anyhow!(
            "launchctl load failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn build_plist(
    program_path: &Path,
    arg_out_dir: &Path,
    weekday: u8,
    hour: u8,
    minute: u8,
) -> Result<Value> {
    anyhow::ensure!(weekday <= 6, "weekday must be 0..=6");
    anyhow::ensure!(hour <= 23, "hour must be 0..=23");
    anyhow::ensure!(minute <= 59, "minute must be 0..=59");

    let mut dict = plist::Dictionary::new();
    dict.insert("Label".into(), LABEL.into());
    dict.insert(
        "ProgramArguments".into(),
        Value::Array(vec![
            Value::String(program_path.to_string_lossy().into_owned()),
            Value::String(crate::safety::SCHEDULED_BACKUP_FLAG.to_string()),
            Value::String(arg_out_dir.to_string_lossy().into_owned()),
        ]),
    );

    let mut sched = plist::Dictionary::new();
    sched.insert("Weekday".into(), Value::Integer((weekday as i64).into()));
    sched.insert("Hour".into(), Value::Integer((hour as i64).into()));
    sched.insert("Minute".into(), Value::Integer((minute as i64).into()));
    dict.insert("StartCalendarInterval".into(), Value::Dictionary(sched));

    dict.insert("RunAtLoad".into(), Value::Boolean(false));
    Ok(Value::Dictionary(dict))
}

pub fn uninstall() -> Result<()> {
    let path = plist_path()?;
    if path.exists() {
        let _ = Command::new("launchctl").arg("unload").arg(&path).output();
        std::fs::remove_file(&path).context("remove plist")?;
    }
    Ok(())
}

pub fn is_installed() -> Result<bool> {
    Ok(plist_path()?.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plist_path_is_under_home_launchagents() {
        let p = plist_path().unwrap();
        assert!(p.to_string_lossy().contains("Library/LaunchAgents"));
        assert!(p
            .to_string_lossy()
            .ends_with("com.hanamorix.manor.snapshot.plist"));
    }

    #[test]
    fn install_rejects_out_of_range_weekday() {
        let tmp = tempdir().unwrap();
        let err = install(tmp.path(), tmp.path(), 8, 2, 0).unwrap_err();
        assert!(err.to_string().contains("weekday"));
    }

    #[test]
    fn install_rejects_out_of_range_hour() {
        let tmp = tempdir().unwrap();
        let err = install(tmp.path(), tmp.path(), 0, 25, 0).unwrap_err();
        assert!(err.to_string().contains("hour"));
    }

    #[test]
    fn plist_invokes_binary_in_scheduled_backup_mode() {
        let tmp = tempdir().unwrap();
        let program = tmp.path().join("Manor.app/Contents/MacOS/manor-desktop");
        let out_dir = tmp.path().join("Backups");
        let plist = build_plist(&program, &out_dir, 1, 2, 3).unwrap();

        let Value::Dictionary(dict) = plist else {
            panic!("plist root should be a dictionary");
        };
        let args = dict
            .get("ProgramArguments")
            .and_then(Value::as_array)
            .expect("ProgramArguments array");

        assert_eq!(
            args[0].as_string(),
            Some(program.to_string_lossy().as_ref())
        );
        assert_eq!(
            args[1].as_string(),
            Some(crate::safety::SCHEDULED_BACKUP_FLAG)
        );
        assert_eq!(
            args[2].as_string(),
            Some(out_dir.to_string_lossy().as_ref())
        );
    }

    // Real `install` + `launchctl load` isn't unit-tested — it'd install a real launch agent
    // on the dev machine. Manual test in Task 9 Step 5.
}
