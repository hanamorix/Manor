//! End-to-end happy path: credentials → first sync → second sync with overlap.
//!
//! Gated behind `--ignored` because it touches the macOS Keychain, which
//! fails on headless CI without an interactive grant.

use manor_app::ledger::{bank_keychain, bank_sync, gocardless};
use manor_core::ledger::bank_account::{self, InsertBankAccount};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
#[ignore = "writes to macOS Keychain; run locally with --ignored"]
async fn end_to_end_happy_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/token/new/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access": "acc-tok",
            "refresh": "ref-tok",
            "access_expires": 86400
        })))
        .mount(&server)
        .await;

    // probe_token endpoint — returns 400 (valid token, invalid country).
    Mock::given(method("GET"))
        .and(path("/api/v2/institutions/"))
        .and(query_param("country", "XX"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&server)
        .await;

    // Transaction fetch — 2 booked rows, 1 pending (to verify pending is filtered).
    Mock::given(method("GET"))
        .and(path("/api/v2/accounts/ext-1/transactions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transactions": {
                "booked": [
                    {
                        "transactionId": "tx-1",
                        "bookingDate": "2026-04-10",
                        "transactionAmount": { "amount": "-12.40", "currency": "GBP" },
                        "creditorName": "TESCO",
                        "remittanceInformationUnstructured": "TESCO STORES"
                    },
                    {
                        "transactionId": "tx-2",
                        "bookingDate": "2026-04-11",
                        "transactionAmount": { "amount": "-5.00", "currency": "GBP" },
                        "creditorName": "COSTA",
                        "remittanceInformationUnstructured": "COSTA COFFEE"
                    }
                ],
                "pending": [
                    {
                        "transactionId": "tx-3",
                        "transactionAmount": { "amount": "-1.00", "currency": "GBP" }
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let client = gocardless::GoCardlessClient::new(server.uri());
    bank_keychain::save_credentials("id", "key").ok();
    client.test_credentials("id", "key").await.unwrap();

    // Fresh core DB.
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let mut conn = manor_core::assistant::db::init(&db_path).unwrap();

    let acct = bank_account::insert(
        &conn,
        InsertBankAccount {
            provider: "gocardless",
            institution_name: "Barclays",
            institution_id: Some("BARCLAYS"),
            institution_logo_url: None,
            account_name: "Current",
            account_type: "current",
            currency: "GBP",
            external_id: "ext-1",
            requisition_id: "req-1",
            reference: "r",
            requisition_created_at: chrono::Utc::now().timestamp() - 86_400,
            requisition_expires_at: chrono::Utc::now().timestamp() + 100_000,
            max_historical_days_granted: 180,
        },
    )
    .unwrap();

    let ctx = bank_sync::SyncContext {
        client: &client,
        allow_rate_limit_bypass: true,
    };

    // First sync — 2 booked rows inserted, pending ignored.
    let report = bank_sync::sync_one(&mut conn, &ctx, acct.id).await.unwrap();
    assert_eq!(report.inserted, 2, "first sync should insert 2 booked rows");
    assert!(!report.skipped);

    // Second sync — same mock response, dedup yields 0 new inserts.
    let report2 = bank_sync::sync_one(&mut conn, &ctx, acct.id).await.unwrap();
    assert_eq!(report2.inserted, 0, "second sync should dedup to 0");

    bank_keychain::wipe_all().ok();
}
