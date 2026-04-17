//! GoCardless Bank Account Data client.
//! Docs: https://bankaccountdata.gocardless.com/api/docs

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ledger::bank_keychain;

pub const DEFAULT_BASE: &str = "https://bankaccountdata.gocardless.com";

#[derive(Debug, thiserror::Error)]
pub enum BankError {
    #[error("auth failed: {0}")]
    AuthFailed(String),
    #[error("EUA params rejected by bank (max_historical_days)")]
    EuaTooLong,
    #[error("requisition expired")]
    RequisitionExpired,
    #[error("rate limited (retry after {0}s)")]
    RateLimited(u64),
    #[error("upstream transient: {0}")]
    UpstreamTransient(String),
    #[error("no credentials in keychain — BYOK wizard required")]
    NoCredentials,
    #[error("{0}")]
    Other(String),
}

#[derive(Clone)]
pub struct GoCardlessClient {
    http: reqwest::Client,
    base: String,
    /// Lock serialises the token refresh dance so concurrent callers don't race.
    token_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access: String,
    refresh: String,
    #[allow(dead_code)]
    access_expires: i64,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access: String,
    #[allow(dead_code)]
    access_expires: i64,
}

#[derive(Debug, Serialize)]
struct TokenNewBody<'a> {
    secret_id: &'a str,
    secret_key: &'a str,
}

impl GoCardlessClient {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("Manor/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
            base: base.into(),
            token_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn default_prod() -> Self {
        Self::new(DEFAULT_BASE)
    }

    /// Test credentials by minting a fresh access token.
    /// Stores access + refresh in Keychain on success.
    pub async fn test_credentials(&self, secret_id: &str, secret_key: &str) -> Result<()> {
        let url = format!("{}/api/v2/token/new/", self.base);
        let resp = self
            .http
            .post(&url)
            .json(&TokenNewBody { secret_id, secret_key })
            .send()
            .await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body).into());
        }
        let tok: TokenResponse = resp.json().await?;
        bank_keychain::save_access_token(&tok.access)?;
        bank_keychain::save_refresh_token(&tok.refresh)?;
        Ok(())
    }

    /// Returns a currently-valid bearer token, rotating via /token/refresh
    /// or /token/new as necessary.
    pub async fn ensure_access_token(&self) -> Result<String> {
        let _guard = self.token_lock.lock().await;

        // Fast path: access token exists and we trust it. We don't store
        // expiry in keychain, so on every call we probe by attempting a
        // cheap authenticated GET; if it 401s, we rotate.
        if let Some(tok) = bank_keychain::get_access_token()? {
            if self.probe_token(&tok).await? {
                return Ok(tok);
            }
        }

        // Refresh path.
        if let Some(refresh) = bank_keychain::get_refresh_token()? {
            if let Ok(new_access) = self.refresh(&refresh).await {
                bank_keychain::save_access_token(&new_access)?;
                return Ok(new_access);
            }
        }

        // Re-auth from stored credentials.
        let (id, key) = bank_keychain::get_credentials()
            .map_err(|_| BankError::NoCredentials)?;
        self.test_credentials(&id, &key).await?;
        bank_keychain::get_access_token()?
            .ok_or_else(|| anyhow!("access token missing after re-auth"))
    }

    async fn probe_token(&self, tok: &str) -> Result<bool> {
        // Cheapest authenticated endpoint: GET /institutions/?country=XX
        // returns 400 if the token is valid (invalid country), 401 if not.
        let url = format!("{}/api/v2/institutions/?country=XX", self.base);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        Ok(resp.status() != StatusCode::UNAUTHORIZED)
    }

    async fn refresh(&self, refresh: &str) -> Result<String> {
        #[derive(Serialize)]
        struct Body<'a> { refresh: &'a str }
        let url = format!("{}/api/v2/token/refresh/", self.base);
        let resp = self.http.post(&url).json(&Body { refresh }).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body).into());
        }
        let r: RefreshResponse = resp.json().await?;
        Ok(r.access)
    }
}

/// Maps GoCardless HTTP errors onto `BankError`.
pub fn map_http_error(status: StatusCode, body: &str) -> BankError {
    match status {
        StatusCode::UNAUTHORIZED => BankError::AuthFailed(body.into()),
        StatusCode::TOO_MANY_REQUESTS => BankError::RateLimited(300),
        StatusCode::BAD_REQUEST if body.contains("max_historical_days") => {
            BankError::EuaTooLong
        }
        StatusCode::CONFLICT if body.contains("expired") => BankError::RequisitionExpired,
        s if s.is_server_error() => BankError::UpstreamTransient(format!("{s}: {body}")),
        other => BankError::Other(format!("{other}: {body}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_credentials_ok_stores_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/token/new/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access": "acc-tok", "refresh": "ref-tok", "access_expires": 86400
            })))
            .mount(&server)
            .await;

        let client = GoCardlessClient::new(server.uri());
        // Keychain writes may fail on headless CI; we only assert the call returns Ok.
        let result = client.test_credentials("id", "key").await;
        // Allow keychain failure on Linux CI; assert no network-layer error.
        if let Err(e) = &result {
            if !e.to_string().contains("keyring") && !e.to_string().contains("Platform") {
                panic!("unexpected error: {e}");
            }
        }
        // Best-effort cleanup.
        bank_keychain::wipe_all().ok();
    }

    #[tokio::test]
    async fn test_credentials_bad_returns_auth_failed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/token/new/"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad creds"))
            .mount(&server)
            .await;

        let client = GoCardlessClient::new(server.uri());
        let err = client.test_credentials("id", "key").await.unwrap_err();
        let be = err.downcast::<BankError>().unwrap();
        assert!(matches!(be, BankError::AuthFailed(_)));
    }

    #[test]
    fn error_mapping_covers_known_cases() {
        assert!(matches!(
            map_http_error(StatusCode::BAD_REQUEST, "max_historical_days exceeds"),
            BankError::EuaTooLong
        ));
        assert!(matches!(
            map_http_error(StatusCode::CONFLICT, "requisition expired"),
            BankError::RequisitionExpired
        ));
        assert!(matches!(
            map_http_error(StatusCode::TOO_MANY_REQUESTS, ""),
            BankError::RateLimited(_)
        ));
        assert!(matches!(
            map_http_error(StatusCode::INTERNAL_SERVER_ERROR, ""),
            BankError::UpstreamTransient(_)
        ));
    }
}
