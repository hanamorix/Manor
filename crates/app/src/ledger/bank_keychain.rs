//! GoCardless Keychain wrapper.
//! Uses service="manor" to stay consistent with CalDAV keychain.
//! Four accounts: gocardless-secret-id, gocardless-secret-key,
//! gocardless-access-token, gocardless-refresh-token.

use anyhow::Result;
use keyring::Entry;

const SERVICE: &str = "manor";
const SECRET_ID: &str = "gocardless-secret-id";
const SECRET_KEY: &str = "gocardless-secret-key";
const ACCESS_TOKEN: &str = "gocardless-access-token";
const REFRESH_TOKEN: &str = "gocardless-refresh-token";

fn entry(account: &str) -> Result<Entry> {
    Ok(Entry::new(SERVICE, account)?)
}

pub fn save_credentials(secret_id: &str, secret_key: &str) -> Result<()> {
    entry(SECRET_ID)?.set_password(secret_id)?;
    entry(SECRET_KEY)?.set_password(secret_key)?;
    Ok(())
}

pub fn has_credentials() -> Result<bool> {
    match entry(SECRET_ID)?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn get_credentials() -> Result<(String, String)> {
    let id = entry(SECRET_ID)?.get_password()?;
    let key = entry(SECRET_KEY)?.get_password()?;
    Ok((id, key))
}

pub fn save_access_token(token: &str) -> Result<()> {
    entry(ACCESS_TOKEN)?.set_password(token)?;
    Ok(())
}

pub fn get_access_token() -> Result<Option<String>> {
    match entry(ACCESS_TOKEN)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn save_refresh_token(token: &str) -> Result<()> {
    entry(REFRESH_TOKEN)?.set_password(token)?;
    Ok(())
}

pub fn get_refresh_token() -> Result<Option<String>> {
    match entry(REFRESH_TOKEN)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Wipe all four entries. Returns how many entries were actually present.
pub fn wipe_all() -> Result<u8> {
    let mut wiped = 0u8;
    for acct in [SECRET_ID, SECRET_KEY, ACCESS_TOKEN, REFRESH_TOKEN] {
        match entry(acct)?.delete_credential() {
            Ok(()) => wiped += 1,
            Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(wiped)
}
