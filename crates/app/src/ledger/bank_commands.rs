//! Autocategorisation command for CSV-imported transactions.
//!
//! This module used to host the GoCardless bank-sync surface; that was retired
//! in v0.1.2. The surviving command uses Ollama to suggest category ids for
//! uncategorised CSV-imported transactions.

use crate::assistant::commands::Db;
use crate::assistant::ollama::{resolve_model, OllamaClient, DEFAULT_ENDPOINT};
use anyhow::Result;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct BankCmdError {
    pub code: String,
    pub message: String,
}

type CmdResult<T> = Result<T, BankCmdError>;

fn err(code: &str, e: impl std::fmt::Display) -> BankCmdError {
    BankCmdError {
        code: code.into(),
        message: e.to_string(),
    }
}

fn map_anyhow(e: anyhow::Error) -> BankCmdError {
    err("other", e)
}

fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in s[start..].char_indices() {
        match ch {
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

#[tauri::command]
pub async fn ledger_bank_autocat_pending(state: State<'_, Db>) -> CmdResult<usize> {
    #[derive(Clone)]
    struct Pending {
        id: i64,
        description: String,
        merchant: Option<String>,
        amount_pence: i64,
    }

    let (pendings, categories, model) = {
        let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
        let cutoff = chrono::Utc::now().timestamp() - 7 * 86_400;
        let mut stmt = conn
            .prepare(
                "SELECT id, description, merchant, amount_pence
                 FROM ledger_transaction
                 WHERE source IN ('csv_import', 'csv_import_legacy')
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
        let model = resolve_model(&conn);
        (rows, cats, model)
    };

    if pendings.is_empty() {
        return Ok(0);
    }

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

    let client = OllamaClient::new(DEFAULT_ENDPOINT, &model);
    let response = match client.complete(&prompt).await {
        Ok(r) => r,
        Err(_) => return Ok(0),
    };

    let json_slice = extract_json_object(&response).unwrap_or("");
    if json_slice.is_empty() {
        return Ok(0);
    }
    let mapping: std::collections::HashMap<String, serde_json::Value> =
        match serde_json::from_str(json_slice) {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };

    let valid_ids: std::collections::HashSet<i64> =
        categories.iter().map(|c| c.id).collect();
    let conn = state.0.lock().map_err(|e| err("lock_poisoned", e))?;
    let mut updated = 0usize;
    for (tx_id_str, cat_val) in mapping {
        let Ok(tx_id) = tx_id_str.parse::<i64>() else {
            continue;
        };
        let Some(cat_id) = cat_val.as_i64() else {
            continue;
        };
        if !valid_ids.contains(&cat_id) {
            continue;
        }
        let n = conn
            .execute(
                "UPDATE ledger_transaction
                 SET category_id = ?1
                 WHERE id = ?2
                   AND category_id IS NULL
                   AND source IN ('csv_import', 'csv_import_legacy')",
                rusqlite::params![cat_id, tx_id],
            )
            .map_err(|e| err("db", e))?;
        updated += n;
    }
    Ok(updated)
}
