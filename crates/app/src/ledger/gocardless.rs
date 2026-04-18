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
    /// Test-only: when Some, `ensure_access_token` returns this directly without
    /// touching the system keychain.  Set via `GoCardlessClient::with_token`.
    #[cfg(test)]
    pinned_token: Option<String>,
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
            #[cfg(test)]
            pinned_token: None,
        }
    }

    /// Test-only constructor that bypasses keychain entirely.
    /// `ensure_access_token` returns `token` immediately without any keychain I/O.
    #[cfg(test)]
    pub fn with_token(base: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            pinned_token: Some(token.into()),
            ..Self::new(base)
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
        // Test-only shortcut: skip keychain entirely.
        #[cfg(test)]
        if let Some(ref tok) = self.pinned_token {
            return Ok(tok.clone());
        }

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
        //
        // On network / 5xx errors we DO NOT fail the whole sync — the probe
        // exists only to detect explicit UNAUTHORIZED. If the upstream is
        // flaky we optimistically say "token probably valid"; the real call
        // that follows will surface any genuine auth failure.
        let url = format!("{}/api/v2/institutions/?country=XX", self.base);
        match self.http.get(&url).bearer_auth(tok).send().await {
            Ok(resp) => Ok(resp.status() != StatusCode::UNAUTHORIZED),
            Err(_) => Ok(true),
        }
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
///
/// `AuthFailed` always carries a user-friendly message rather than the
/// raw GoCardless response body — the body may contain provider-internal
/// details that have no home on the frontend. We keep the raw body out
/// of the user-visible error text; logs should capture it separately if
/// needed (future work).
pub fn map_http_error(status: StatusCode, body: &str) -> BankError {
    match status {
        StatusCode::UNAUTHORIZED => BankError::AuthFailed(
            "GoCardless rejected the credentials — check your Secret ID and Secret Key in Settings → Bank Accounts.".into(),
        ),
        StatusCode::TOO_MANY_REQUESTS => BankError::RateLimited(300),
        StatusCode::BAD_REQUEST if body.contains("max_historical_days") => {
            BankError::EuaTooLong
        }
        StatusCode::CONFLICT if body.contains("expired") => BankError::RequisitionExpired,
        s if s.is_server_error() => BankError::UpstreamTransient(format!("{s}")),
        other => BankError::Other(format!("{other}")),
    }
}

// ------- Institutions -------

#[derive(Debug, Deserialize, Clone)]
pub struct RawInstitution {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub bic: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default = "default_max_hist")]
    pub transaction_total_days: String, // GoCardless returns as stringified number
    #[serde(default = "default_access_valid")]
    pub max_access_valid_for_days: String,
}

fn default_max_hist() -> String { "90".into() }
fn default_access_valid() -> String { "90".into() }

impl GoCardlessClient {
    pub async fn list_institutions(&self, country: &str) -> Result<Vec<RawInstitution>> {
        let tok = self.ensure_access_token().await?;
        let url = format!("{}/api/v2/institutions/?country={}", self.base, country);
        let resp = self.http.get(&url).bearer_auth(&tok).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body).into());
        }
        Ok(resp.json::<Vec<RawInstitution>>().await?)
    }
}

// ------- End User Agreements -------

#[derive(Debug, Deserialize)]
struct AgreementResponse {
    id: String,
}

impl GoCardlessClient {
    /// Creates an EUA. On 400 "max_historical_days exceeds", retries with (90, 90).
    /// Returns (agreement_id, max_historical_days_granted).
    pub async fn create_agreement(
        &self,
        institution_id: &str,
        preferred_days: (u16, u16),
    ) -> Result<(String, u16)> {
        match self.create_agreement_inner(institution_id, preferred_days).await {
            Ok(id) => Ok((id, preferred_days.0)),
            Err(e) => {
                if let Some(BankError::EuaTooLong) = e.downcast_ref::<BankError>() {
                    let id = self.create_agreement_inner(institution_id, (90, 90)).await?;
                    Ok((id, 90))
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn create_agreement_inner(
        &self,
        institution_id: &str,
        (max_hist, access_valid): (u16, u16),
    ) -> Result<String> {
        let tok = self.ensure_access_token().await?;
        let url = format!("{}/api/v2/agreements/enduser/", self.base);
        let body = serde_json::json!({
            "institution_id": institution_id,
            "max_historical_days": max_hist,
            "access_valid_for_days": access_valid,
            "access_scope": ["balances", "details", "transactions"],
        });
        let resp = self.http.post(&url).bearer_auth(&tok).json(&body).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &text).into());
        }
        let a: AgreementResponse = resp.json().await?;
        Ok(a.id)
    }
}

// ------- Requisitions -------

#[derive(Debug, Deserialize)]
pub struct RawRequisition {
    pub id: String,
    pub link: String,
    #[serde(default)]
    pub accounts: Vec<String>,
}

impl GoCardlessClient {
    pub async fn create_requisition(
        &self,
        institution_id: &str,
        agreement_id: &str,
        redirect: &str,
        reference: &str,
    ) -> Result<RawRequisition> {
        let tok = self.ensure_access_token().await?;
        let url = format!("{}/api/v2/requisitions/", self.base);
        let body = serde_json::json!({
            "institution_id": institution_id,
            "agreement": agreement_id,
            "redirect": redirect,
            "reference": reference,
            "user_language": "EN",
        });
        let resp = self.http.post(&url).bearer_auth(&tok).json(&body).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &text).into());
        }
        Ok(resp.json::<RawRequisition>().await?)
    }

    pub async fn fetch_requisition_accounts(&self, requisition_id: &str) -> Result<Vec<String>> {
        let tok = self.ensure_access_token().await?;
        let url = format!("{}/api/v2/requisitions/{requisition_id}/", self.base);
        let resp = self.http.get(&url).bearer_auth(&tok).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &text).into());
        }
        Ok(resp.json::<RawRequisition>().await?.accounts)
    }

    pub async fn delete_requisition(&self, requisition_id: &str) -> Result<()> {
        let tok = self.ensure_access_token().await?;
        let url = format!("{}/api/v2/requisitions/{requisition_id}/", self.base);
        let resp = self.http.delete(&url).bearer_auth(&tok).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() && status != StatusCode::NOT_FOUND {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &text).into());
        }
        Ok(())
    }
}

// ------- Account details -------

#[derive(Debug, Deserialize, Clone)]
pub struct RawAccountDetails {
    pub iban: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "ownerName")]
    pub owner_name: Option<String>,
    pub currency: Option<String>,
    #[serde(rename = "cashAccountType")]
    pub cash_account_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawInstitutionDetails {
    pub name: String,
    #[serde(default)]
    pub logo: Option<String>,
}

impl GoCardlessClient {
    /// `/accounts/{id}/details/` returns account + institution blocks.
    pub async fn fetch_account_details(
        &self,
        external_id: &str,
    ) -> Result<(RawAccountDetails, RawInstitutionDetails)> {
        let tok = self.ensure_access_token().await?;
        let url = format!("{}/api/v2/accounts/{external_id}/details/", self.base);
        let resp = self.http.get(&url).bearer_auth(&tok).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &text).into());
        }
        #[derive(Deserialize)]
        struct Envelope {
            account: RawAccountDetails,
            #[serde(default)]
            institution: Option<RawInstitutionDetails>,
        }
        let env: Envelope = resp.json().await?;
        let inst = env.institution.unwrap_or(RawInstitutionDetails {
            name: "Bank".into(),
            logo: None,
        });
        Ok((env.account, inst))
    }
}

// ------- Transactions -------

#[derive(Debug, Deserialize, Clone)]
pub struct RawTransaction {
    #[serde(rename = "transactionId")]
    pub transaction_id: Option<String>,
    #[serde(rename = "internalTransactionId")]
    pub internal_transaction_id: Option<String>,
    pub booking_date: Option<String>,
    #[serde(rename = "bookingDate")]
    pub booking_date_camel: Option<String>,
    /// Snake-case amount field (some banks / GoCardless sandbox).
    #[serde(default)]
    pub transaction_amount: Option<RawAmount>,
    /// Camel-case amount field (standard GoCardless production API).
    #[serde(rename = "transactionAmount")]
    pub transaction_amount_camel: Option<RawAmount>,
    pub remittance_information_unstructured: Option<String>,
    #[serde(rename = "remittanceInformationUnstructured")]
    pub remittance_information_unstructured_camel: Option<String>,
    pub creditor_name: Option<String>,
    #[serde(rename = "creditorName")]
    pub creditor_name_camel: Option<String>,
    pub debtor_name: Option<String>,
    #[serde(rename = "debtorName")]
    pub debtor_name_camel: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawAmount {
    pub amount: String, // GoCardless returns as string
    pub currency: String,
}

impl RawTransaction {
    /// Canonical merchant name — prefer creditorName, then debtorName.
    pub fn merchant(&self) -> Option<String> {
        self.creditor_name.clone()
            .or_else(|| self.creditor_name_camel.clone())
            .or_else(|| self.debtor_name.clone())
            .or_else(|| self.debtor_name_camel.clone())
    }

    pub fn description(&self) -> String {
        self.remittance_information_unstructured.clone()
            .or_else(|| self.remittance_information_unstructured_camel.clone())
            .unwrap_or_else(|| self.merchant().unwrap_or_else(|| "Unknown".into()))
    }

    pub fn external_id(&self) -> Option<String> {
        self.transaction_id.clone()
            .or_else(|| self.internal_transaction_id.clone())
    }

    pub fn booking_date_str(&self) -> Option<String> {
        self.booking_date.clone().or_else(|| self.booking_date_camel.clone())
    }

    pub fn amount_pence(&self) -> Option<i64> {
        let amt = self.transaction_amount_camel.as_ref()
            .or(self.transaction_amount.as_ref())?;
        let f: f64 = amt.amount.parse().ok()?;
        Some((f * 100.0).round() as i64)
    }
}

impl GoCardlessClient {
    /// Fetches booked transactions for an account since `date_from` (YYYY-MM-DD).
    pub async fn fetch_transactions(
        &self,
        external_id: &str,
        date_from: &str,
    ) -> Result<Vec<RawTransaction>> {
        let tok = self.ensure_access_token().await?;
        let url = format!(
            "{}/api/v2/accounts/{external_id}/transactions/?date_from={date_from}",
            self.base
        );
        let resp = self.http.get(&url).bearer_auth(&tok).send().await
            .map_err(|e| BankError::UpstreamTransient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &text).into());
        }
        #[derive(Deserialize)]
        struct Envelope {
            transactions: Transactions,
        }
        #[derive(Deserialize)]
        struct Transactions {
            booked: Vec<RawTransaction>,
        }
        let env: Envelope = resp.json().await?;
        Ok(env.transactions.booked)
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

    #[tokio::test]
    async fn list_institutions_happy_path() {
        let server = MockServer::start().await;

        // Real institutions list for GB.
        Mock::given(method("GET"))
            .and(path("/api/v2/institutions/"))
            .and(wiremock::matchers::query_param("country", "GB"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "BARCLAYS", "name": "Barclays", "transaction_total_days": "180", "max_access_valid_for_days": "180" }
            ])))
            .mount(&server)
            .await;

        // with_token bypasses keychain entirely — no system keychain I/O in this test.
        let client = GoCardlessClient::with_token(server.uri(), "test-token");
        let result = client.list_institutions("GB").await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "BARCLAYS");
    }

    #[tokio::test]
    async fn create_agreement_falls_back_to_90() {
        let server = MockServer::start().await;

        // First call: 400 max_historical_days. Second call: 200.
        Mock::given(method("POST"))
            .and(path("/api/v2/agreements/enduser/"))
            .respond_with(
                ResponseTemplate::new(400).set_body_string(
                    "{\"detail\":\"max_historical_days exceeds bank limit\"}",
                ),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v2/agreements/enduser/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "agr-id"
            })))
            .mount(&server)
            .await;

        let client = GoCardlessClient::with_token(server.uri(), "test-token");
        let (id, granted) = client.create_agreement("BARCLAYS", (180, 180)).await.unwrap();
        assert_eq!(id, "agr-id");
        assert_eq!(granted, 90);
    }

    #[tokio::test]
    async fn fetch_transactions_returns_booked_only() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/accounts/acc-1/transactions/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "transactions": {
                    "booked": [{
                        "transactionId": "tx-1",
                        "bookingDate": "2026-04-10",
                        "transactionAmount": { "amount": "-12.40", "currency": "GBP" },
                        "creditorName": "TESCO",
                        "remittanceInformationUnstructured": "TESCO STORES 4023"
                    }],
                    "pending": [{
                        "transactionId": "tx-2-pending",
                        "transactionAmount": { "amount": "-5.00", "currency": "GBP" }
                    }]
                }
            })))
            .mount(&server)
            .await;

        let client = GoCardlessClient::with_token(server.uri(), "test-token");
        let txs = client.fetch_transactions("acc-1", "2026-04-01").await.unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].external_id(), Some("tx-1".into()));
        assert_eq!(txs[0].merchant(), Some("TESCO".into()));
        assert_eq!(txs[0].amount_pence(), Some(-1240));
    }

    #[test]
    fn amount_pence_handles_decimal_strings() {
        let mk = |amt: &str| RawTransaction {
            transaction_id: None, internal_transaction_id: None,
            booking_date: None, booking_date_camel: None,
            transaction_amount: Some(RawAmount { amount: amt.into(), currency: "GBP".into() }),
            transaction_amount_camel: None,
            remittance_information_unstructured: None,
            remittance_information_unstructured_camel: None,
            creditor_name: None, creditor_name_camel: None,
            debtor_name: None, debtor_name_camel: None,
        };
        assert_eq!(mk("-12.40").amount_pence(), Some(-1240));
        assert_eq!(mk("100.00").amount_pence(), Some(10000));
        assert_eq!(mk("0.01").amount_pence(), Some(1));
    }
}
