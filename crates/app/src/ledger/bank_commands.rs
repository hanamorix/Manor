//! Tauri commands for Phase 5d bank sync.

use anyhow::Result;
use manor_core::ledger::{bank_account, institution_cache};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::assistant::commands::Db;
use crate::ledger::{bank_keychain, bank_sync, gocardless, oauth_server};

#[derive(Debug, Serialize)]
pub struct BankCmdError {
    pub code: String,
    pub message: String,
}

type CmdResult<T> = Result<T, BankCmdError>;

fn err(code: &str, e: impl std::fmt::Display) -> BankCmdError {
    BankCmdError { code: code.into(), message: e.to_string() }
}

fn map_anyhow(e: anyhow::Error) -> BankCmdError {
    if let Some(be) = e.downcast_ref::<gocardless::BankError>() {
        let code = match be {
            gocardless::BankError::AuthFailed(_) => "auth_failed",
            gocardless::BankError::EuaTooLong => "eua_too_long",
            gocardless::BankError::RequisitionExpired => "requisition_expired",
            gocardless::BankError::RateLimited(_) => "rate_limited",
            gocardless::BankError::UpstreamTransient(_) => "upstream_transient",
            gocardless::BankError::NoCredentials => "no_credentials",
            gocardless::BankError::Other(_) => "other",
        };
        BankCmdError { code: code.into(), message: be.to_string() }
    } else {
        err("other", e)
    }
}

#[tauri::command]
pub async fn ledger_bank_credentials_status() -> CmdResult<bool> {
    bank_keychain::has_credentials().map_err(map_anyhow)
}

#[derive(Deserialize)]
pub struct SaveCredsArgs {
    pub secret_id: String,
    pub secret_key: String,
}

#[tauri::command]
pub async fn ledger_bank_save_credentials(args: SaveCredsArgs) -> CmdResult<()> {
    bank_keychain::save_credentials(&args.secret_id, &args.secret_key)
        .map_err(map_anyhow)?;
    let client = gocardless::GoCardlessClient::default_prod();
    client.test_credentials(&args.secret_id, &args.secret_key)
        .await
        .map_err(map_anyhow)?;
    Ok(())
}

#[derive(Serialize)]
pub struct UiInstitution {
    pub id: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub is_sandbox: bool,
}

#[tauri::command]
pub async fn ledger_bank_list_institutions(
    state: State<'_, Db>,
    country: String,
) -> CmdResult<Vec<UiInstitution>> {
    // Fresh cache lookup — needs only a shared borrow.
    let cached = {
        let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        institution_cache::get_fresh(&conn, &country).map_err(map_anyhow)?
    };

    let rows: Vec<institution_cache::CachedInstitution> = if cached.is_empty() {
        let client = gocardless::GoCardlessClient::default_prod();
        let raw = client.list_institutions(&country).await.map_err(map_anyhow)?;
        let mapped: Vec<_> = raw.into_iter().map(|r| {
            institution_cache::CachedInstitution {
                country: country.clone(),
                institution_id: r.id,
                name: r.name,
                bic: r.bic,
                logo_url: r.logo,
                max_historical_days: r.transaction_total_days.parse().unwrap_or(90),
                access_valid_for_days: r.max_access_valid_for_days.parse().unwrap_or(90),
            }
        }).collect();
        // replace_for_country needs &mut Connection.
        let mut conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        institution_cache::replace_for_country(&mut conn, &country, &mapped)
            .map_err(map_anyhow)?;
        mapped
    } else {
        cached
    };

    // Sort alphabetically (get_fresh already orders by name, but mapped is unordered).
    let mut rows = rows;
    rows.sort_by(|a, b| a.name.cmp(&b.name));

    let mut out: Vec<UiInstitution> = rows.into_iter().map(|r| UiInstitution {
        id: r.institution_id,
        name: r.name,
        logo_url: r.logo_url,
        is_sandbox: false,
    }).collect();

    let sandbox_on: bool = {
        let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        conn.query_row(
            "SELECT value FROM setting WHERE key = 'bank_sandbox_enabled'",
            [], |r| r.get::<_, String>(0),
        ).map(|v| v == "true").unwrap_or(false)
    };
    if sandbox_on {
        out.insert(0, UiInstitution {
            id: "SANDBOXFINANCE_SFIN0000".into(),
            name: "SANDBOX (test institution)".into(),
            logo_url: None,
            is_sandbox: true,
        });
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct BeginConnectResponse {
    pub auth_url: String,
    pub requisition_id: String,
    pub reference: String,
    pub port: u16,
    pub max_historical_days_granted: i64,
    pub institution_id: String,
}

pub type PendingCallbacks = Arc<Mutex<HashMap<String, oauth_server::LoopbackCallback>>>;

#[derive(Deserialize)]
pub struct BeginConnectArgs {
    pub institution_id: String,
}

#[tauri::command]
pub async fn ledger_bank_begin_connect(
    callbacks: State<'_, PendingCallbacks>,
    args: BeginConnectArgs,
) -> CmdResult<BeginConnectResponse> {
    let cb = oauth_server::start().map_err(map_anyhow)?;
    let port = cb.port;
    let redirect = format!("http://127.0.0.1:{port}/bank-auth");
    let reference = Uuid::new_v4().to_string();

    let client = gocardless::GoCardlessClient::default_prod();
    let (agreement_id, granted) = client
        .create_agreement(&args.institution_id, (180, 180))
        .await
        .map_err(map_anyhow)?;
    let req = client
        .create_requisition(&args.institution_id, &agreement_id, &redirect, &reference)
        .await
        .map_err(map_anyhow)?;

    callbacks.lock().await.insert(reference.clone(), cb);

    Ok(BeginConnectResponse {
        auth_url: req.link,
        requisition_id: req.id,
        reference,
        port,
        max_historical_days_granted: granted as i64,
        institution_id: args.institution_id,
    })
}

#[derive(Deserialize)]
pub struct CompleteConnectArgs {
    pub reference: String,
    pub requisition_id: String,
    pub institution_id: String,
    pub institution_name: String,
    pub institution_logo_url: Option<String>,
    pub max_historical_days_granted: i64,
    /// When set, soft-delete this account after the new rows are inserted.
    /// Used by the reconnect flow to replace an expired account cleanly.
    #[serde(default)]
    pub replaces_account_id: Option<i64>,
}

#[derive(Serialize)]
pub struct CompleteConnectResponse {
    pub account_ids: Vec<i64>,
}

#[tauri::command]
pub async fn ledger_bank_complete_connect(
    state: State<'_, Db>,
    callbacks: State<'_, PendingCallbacks>,
    args: CompleteConnectArgs,
) -> CmdResult<CompleteConnectResponse> {
    let cb_opt = callbacks.lock().await.remove(&args.reference);
    let cb = cb_opt.ok_or_else(|| err("no_pending_callback", "no pending callback for reference"))?;
    let _params = cb.receiver.await
        .map_err(|e| err("oauth_channel_dropped", e))?
        .map_err(map_anyhow)?;

    let client = gocardless::GoCardlessClient::default_prod();
    let externals = client
        .fetch_requisition_accounts(&args.requisition_id)
        .await
        .map_err(map_anyhow)?;

    let now = chrono::Utc::now().timestamp();
    let expires_at = now + args.max_historical_days_granted * 86_400;

    let mut ids = Vec::with_capacity(externals.len());
    for ext in &externals {
        let (details, inst) = client.fetch_account_details(ext).await.map_err(map_anyhow)?;
        let name = details.name.clone()
            .or(details.owner_name.clone())
            .unwrap_or_else(|| "Account".into());
        let currency = details.currency.clone().unwrap_or_else(|| "GBP".into());
        let acct_type = details.cash_account_type.clone().unwrap_or_else(|| "current".into());

        let inserted = {
            let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
            bank_account::insert(&conn, bank_account::InsertBankAccount {
                provider: "gocardless",
                institution_name: &args.institution_name,
                institution_id: Some(&args.institution_id),
                institution_logo_url: args.institution_logo_url.as_deref()
                    .or(inst.logo.as_deref()),
                account_name: &name,
                account_type: &acct_type,
                currency: &currency,
                external_id: ext,
                requisition_id: &args.requisition_id,
                reference: &args.reference,
                requisition_created_at: now,
                requisition_expires_at: expires_at,
                max_historical_days_granted: args.max_historical_days_granted,
            }).map_err(map_anyhow)?
        };
        ids.push(inserted.id);
    }

    // If this was a reconnect, soft-delete the old account row now that
    // replacement rows are in place. Fails softly — we don't want a stale
    // row to block the happy path if the delete errors.
    if let Some(old_id) = args.replaces_account_id {
        if let Ok(conn) = state.0.lock() {
            let _ = bank_account::soft_delete(&conn, old_id);
        }
    }

    // Clone the Arc so the background sync task owns it; no lock held across .await.
    let db_for_sync = state.0.clone();
    let handle = tokio::runtime::Handle::current();
    tauri::async_runtime::spawn_blocking(move || {
        if let Ok(mut conn) = db_for_sync.lock() {
            let client = gocardless::GoCardlessClient::default_prod();
            let ctx = bank_sync::SyncContext { client: &client, allow_rate_limit_bypass: true };
            let _ = handle.block_on(bank_sync::sync_all(&mut conn, &ctx));
        }
    });

    Ok(CompleteConnectResponse { account_ids: ids })
}

#[tauri::command]
pub async fn ledger_bank_list_accounts(
    state: State<'_, Db>,
) -> CmdResult<Vec<bank_account::BankAccount>> {
    let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
    bank_account::list(&conn).map_err(map_anyhow)
}

#[derive(Deserialize)]
pub struct SyncNowArgs {
    pub account_id: Option<i64>,
}

#[tauri::command]
pub async fn ledger_bank_sync_now(
    state: State<'_, Db>,
    args: SyncNowArgs,
) -> CmdResult<Vec<bank_sync::SyncAccountReport>> {
    // Mirror the spawn_blocking + block_on pattern used by complete_connect
    // and the CalDAV sync_account command in lib.rs — std::sync::MutexGuard
    // is !Send, so holding it across an .await is not portable.
    let db = state.0.clone();
    let account_id = args.account_id;
    let handle = tokio::runtime::Handle::current();
    let join = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<Vec<bank_sync::SyncAccountReport>> {
        let client = gocardless::GoCardlessClient::default_prod();
        let ctx = bank_sync::SyncContext { client: &client, allow_rate_limit_bypass: true };
        let mut conn = db.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        match account_id {
            Some(id) => Ok(vec![handle.block_on(bank_sync::sync_one(&mut conn, &ctx, id))?]),
            None => handle.block_on(bank_sync::sync_all(&mut conn, &ctx)),
        }
    });
    join.await
        .map_err(|e| err("join_error", e))?
        .map_err(map_anyhow)
}

#[derive(Deserialize)]
pub struct DisconnectArgs {
    pub account_id: i64,
}

#[tauri::command]
pub async fn ledger_bank_disconnect(
    state: State<'_, Db>,
    args: DisconnectArgs,
) -> CmdResult<()> {
    let acct = {
        let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        bank_account::get(&conn, args.account_id).map_err(map_anyhow)?
    };
    if let Some(req_id) = &acct.requisition_id {
        let client = gocardless::GoCardlessClient::default_prod();
        let _ = client.delete_requisition(req_id).await;
    }
    {
        let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        bank_account::soft_delete(&conn, args.account_id).map_err(map_anyhow)?;
        let remaining = bank_account::list(&conn).map_err(map_anyhow)?.len();
        if remaining == 0 {
            bank_keychain::wipe_all().map_err(map_anyhow)?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct ReconnectArgs {
    pub account_id: i64,
}

#[tauri::command]
pub async fn ledger_bank_reconnect(
    state: State<'_, Db>,
    callbacks: State<'_, PendingCallbacks>,
    args: ReconnectArgs,
) -> CmdResult<BeginConnectResponse> {
    let inst_id = {
        let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        bank_account::get(&conn, args.account_id)
            .map_err(map_anyhow)?
            .institution_id
            .ok_or_else(|| err("no_institution", "missing institution_id"))?
    };
    ledger_bank_begin_connect(callbacks, BeginConnectArgs { institution_id: inst_id }).await
}

#[derive(Deserialize)]
pub struct CancelConnectArgs {
    pub reference: String,
}

#[tauri::command]
pub async fn ledger_bank_cancel_connect(
    callbacks: State<'_, PendingCallbacks>,
    args: CancelConnectArgs,
) -> CmdResult<()> {
    // Signal the listener thread to release the port within ~500ms, then
    // drop the callback entry so a subsequent begin_connect gets a fresh
    // slot. The resolved-Err on the receiver is discarded — nobody is
    // awaiting it once the drawer has dismissed.
    let mut map = callbacks.lock().await;
    if let Some(cb) = map.remove(&args.reference) {
        cb.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn ledger_bank_autocat_pending(
    state: State<'_, Db>,
) -> CmdResult<usize> {
    use crate::assistant::ollama::{OllamaClient, DEFAULT_ENDPOINT, DEFAULT_MODEL};

    // 1. Collect uncategorised sync rows < 7 days old (hot window), limit 50.
    #[derive(Clone)]
    struct Pending {
        id: i64,
        description: String,
        merchant: Option<String>,
        amount_pence: i64,
    }

    let (pendings, categories) = {
        let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        let cutoff = chrono::Utc::now().timestamp() - 7 * 86_400;
        let mut stmt = conn
            .prepare(
                "SELECT id, description, merchant, amount_pence
                 FROM ledger_transaction
                 WHERE source = 'sync'
                   AND category_id IS NULL
                   AND deleted_at IS NULL
                   AND date >= ?1
                 ORDER BY date DESC
                 LIMIT 50",
            )
            .map_err(|e| err("db", e))?;
        let rows: Vec<Pending> = stmt
            .query_map([cutoff], |r| {
                Ok(Pending {
                    id: r.get(0)?,
                    description: r.get(1)?,
                    merchant: r.get(2)?,
                    amount_pence: r.get(3)?,
                })
            })
            .map_err(|e| err("db", e))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| err("db", e))?;

        let cats = manor_core::ledger::category::list(&conn).map_err(map_anyhow)?;
        (rows, cats)
    };

    if pendings.is_empty() {
        return Ok(0);
    }

    // 2. Build a single prompt listing the transactions + category options.
    let cat_list: String = categories
        .iter()
        .filter(|c| c.deleted_at.is_none())
        .map(|c| format!("{}={}", c.id, c.name))
        .collect::<Vec<_>>()
        .join(", ");

    let tx_list: String = pendings
        .iter()
        .map(|p| {
            let merchant = p.merchant.as_deref().unwrap_or("");
            let amount = format!("£{:.2}", (p.amount_pence as f64).abs() / 100.0);
            format!(
                "  {} | merchant: {:?} | desc: {:?} | amount: {}",
                p.id, merchant, p.description, amount
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "You are categorising bank transactions for a UK personal ledger.\n\
         Categories (id=name): {cat_list}\n\
         \n\
         For each transaction below, pick the single best category id. \
         If nothing fits, use the 'Other' category id. Reply ONLY with a \
         compact JSON object mapping transaction id → category id, e.g. \
         {{\"123\":4,\"124\":2}}. No prose, no markdown.\n\
         \n\
         Transactions:\n{tx_list}\n"
    );

    // 3. Call Ollama (non-streaming). If Ollama is unreachable, no-op.
    let client = OllamaClient::new(DEFAULT_ENDPOINT, DEFAULT_MODEL);
    let response = match client.complete(&prompt).await {
        Ok(r) => r,
        Err(_) => return Ok(0),
    };

    // 4. Extract the JSON object (model may wrap it in prose despite our
    //    instructions). Greedy: find the first { and its matching }.
    let json_slice = extract_json_object(&response).unwrap_or("");
    if json_slice.is_empty() {
        return Ok(0);
    }
    let mapping: std::collections::HashMap<String, serde_json::Value> =
        match serde_json::from_str(json_slice) {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };

    // 5. Write categories back.
    let valid_ids: std::collections::HashSet<i64> =
        categories.iter().map(|c| c.id).collect();
    let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
    let mut updated = 0usize;
    for (tx_id_str, cat_val) in mapping {
        let Ok(tx_id) = tx_id_str.parse::<i64>() else { continue };
        let Some(cat_id) = cat_val.as_i64() else { continue };
        if !valid_ids.contains(&cat_id) {
            continue;
        }
        let rows = conn
            .execute(
                "UPDATE ledger_transaction
                 SET category_id = ?1
                 WHERE id = ?2 AND category_id IS NULL AND source = 'sync'",
                rusqlite::params![cat_id, tx_id],
            )
            .unwrap_or(0);
        updated += rows as usize;
    }
    Ok(updated)
}

/// Greedy matched-braces JSON-object extractor — tolerates model prose
/// around the payload ("Here's the mapping: { ... }").
fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let mut depth = 0i32;
    for (i, c) in s[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=start + i]);
                }
            }
            _ => {}
        }
    }
    None
}
