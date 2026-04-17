//! macOS Keychain wrapper for remote provider API keys.
//!
//! Keychain footprint: service="manor-remote", account="{provider}-api-key".
//! Inspectable via Keychain.app so users can see exactly what Manor stores.

use anyhow::Result;
use keyring::Entry;

const SERVICE: &str = "manor-remote";

fn account(provider: &str) -> String {
    format!("{provider}-api-key")
}

pub fn set_key(provider: &str, key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &account(provider))?;
    entry.set_password(key)?;
    Ok(())
}

pub fn get_key(provider: &str) -> Result<Option<String>> {
    let entry = Entry::new(SERVICE, &account(provider))?;
    match entry.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn remove_key(provider: &str) -> Result<bool> {
    let entry = Entry::new(SERVICE, &account(provider))?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn has_key(provider: &str) -> bool {
    get_key(provider).ok().flatten().is_some()
}
