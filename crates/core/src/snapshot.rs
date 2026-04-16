//! Weekly encrypted snapshot backups. Writes `<dir>/manor-YYYY-WW.lifebackup` —
//! age-encrypted tar.gz containing `manor.db` + `attachments/`.

use age::secrecy::SecretString;
use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, Utc};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// File extension used for snapshots.
pub const EXT: &str = "lifebackup";

/// Compose the conventional filename for a backup taken now.
pub fn default_filename(now: chrono::DateTime<Utc>) -> String {
    let iso = now.iso_week();
    format!("manor-{}-W{:02}.{EXT}", iso.year(), iso.week())
}

/// Create a backup archive.
/// * `db_path` — path to `manor.db`
/// * `attachments_dir` — path to attachments directory (may not exist)
/// * `out_path` — destination for the `.lifebackup` file
/// * `passphrase` — the passphrase the user chose for backups
pub fn create(
    db_path: &Path,
    attachments_dir: &Path,
    out_path: &Path,
    passphrase: &str,
) -> Result<()> {
    anyhow::ensure!(db_path.exists(), "db file missing: {}", db_path.display());
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Step 1: build a gzipped tar in memory.
    let mut tar_buf = Vec::<u8>::new();
    {
        let gz = GzEncoder::new(&mut tar_buf, Compression::default());
        let mut tar = tar::Builder::new(gz);
        tar.append_path_with_name(db_path, "manor.db")?;
        if attachments_dir.exists() {
            tar.append_dir_all("attachments", attachments_dir)?;
        }
        tar.finish()?;
    }

    // Step 2: age-encrypt with passphrase.
    let secret = SecretString::from(passphrase.to_string());
    let encryptor = age::Encryptor::with_user_passphrase(secret);
    let out = File::create(out_path).with_context(|| format!("create {}", out_path.display()))?;
    let mut writer = encryptor.wrap_output(out)?;
    writer.write_all(&tar_buf)?;
    writer.finish()?;
    Ok(())
}

/// Restore a snapshot into a staging directory. Caller is responsible for
/// moving the extracted files into place (e.g., replacing the live data dir).
/// Returns the staging directory path.
pub fn restore_to_staging(
    backup_path: &Path,
    staging_dir: &Path,
    passphrase: &str,
) -> Result<PathBuf> {
    std::fs::create_dir_all(staging_dir)?;

    // Step 1: age-decrypt.
    let f = File::open(backup_path).with_context(|| format!("open {}", backup_path.display()))?;
    let decryptor = age::Decryptor::new(f)?;

    let mut decoded = Vec::<u8>::new();
    match decryptor {
        age::Decryptor::Passphrase(d) => {
            let secret = SecretString::from(passphrase.to_string());
            let mut reader = d
                .decrypt(&secret, None)
                .map_err(|e| anyhow!("decrypt failed (wrong passphrase?): {e}"))?;
            reader.read_to_end(&mut decoded)?;
        }
        _ => {
            return Err(anyhow!(
                "backup not passphrase-encrypted (unexpected format)"
            ))
        }
    }

    // Step 2: gunzip + untar into staging.
    let gz = GzDecoder::new(&decoded[..]);
    let mut tar = tar::Archive::new(gz);
    tar.unpack(staging_dir)?;
    Ok(staging_dir.to_path_buf())
}

/// List all `.lifebackup` files in a directory, sorted newest-first by mtime.
pub fn list(dir: &Path) -> Result<Vec<(PathBuf, i64)>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some(EXT) {
            let mtime = entry
                .metadata()?
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.push((path, mtime));
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_filename_uses_iso_week() {
        let d = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
            chrono::NaiveDate::from_ymd_opt(2026, 4, 16)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
            Utc,
        );
        let name = default_filename(d);
        assert!(name.starts_with("manor-2026-W"));
        assert!(name.ends_with(".lifebackup"));
    }

    #[test]
    fn create_and_restore_roundtrip() {
        let tmp = tempdir().unwrap();
        let db = tmp.path().join("manor.db");
        std::fs::write(&db, b"fake db bytes").unwrap();
        let att = tmp.path().join("attachments");
        std::fs::create_dir_all(&att).unwrap();
        std::fs::write(att.join("abc"), b"hello world").unwrap();

        let out = tmp.path().join("snap.lifebackup");
        create(&db, &att, &out, "super-secret").unwrap();
        assert!(out.exists());

        let staging = tmp.path().join("restore");
        restore_to_staging(&out, &staging, "super-secret").unwrap();

        let restored_db = std::fs::read(staging.join("manor.db")).unwrap();
        assert_eq!(restored_db, b"fake db bytes");
        let restored_att = std::fs::read(staging.join("attachments").join("abc")).unwrap();
        assert_eq!(restored_att, b"hello world");
    }

    #[test]
    fn restore_rejects_wrong_passphrase() {
        let tmp = tempdir().unwrap();
        let db = tmp.path().join("manor.db");
        std::fs::write(&db, b"x").unwrap();
        let out = tmp.path().join("s.lifebackup");
        create(&db, &tmp.path().join("nope"), &out, "right").unwrap();
        let staging = tmp.path().join("r");
        let err = restore_to_staging(&out, &staging, "wrong").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("decrypt"));
    }

    #[test]
    fn list_returns_backups_newest_first() {
        let tmp = tempdir().unwrap();
        let a = tmp.path().join("a.lifebackup");
        let b = tmp.path().join("b.lifebackup");
        std::fs::write(&a, b"1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&b, b"2").unwrap();
        std::fs::write(tmp.path().join("c.txt"), b"not a backup").unwrap();
        let items = list(tmp.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0.file_name().unwrap(), "b.lifebackup");
    }
}
