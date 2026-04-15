//! macOS Keychain wrapper for CalDAV passwords.
//!
//! `keyring` is cross-platform but defaults to the macOS keychain on Mac.
//! Entries are keyed by (service="manor", account="caldav-{account_id}").

use anyhow::Result;
use keyring::Entry;

const SERVICE: &str = "manor";

fn account_key(account_id: i64) -> String {
    format!("caldav-{account_id}")
}

pub fn set_password(account_id: i64, password: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &account_key(account_id))?;
    entry.set_password(password)?;
    Ok(())
}

pub fn get_password(account_id: i64) -> Result<String> {
    let entry = Entry::new(SERVICE, &account_key(account_id))?;
    Ok(entry.get_password()?)
}

/// Delete the Keychain entry. Missing entries are not an error — they're reported
/// as `Ok(false)` so callers know whether an entry actually existed.
pub fn delete_password(account_id: i64) -> Result<bool> {
    let entry = Entry::new(SERVICE, &account_key(account_id))?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.into()),
    }
}
