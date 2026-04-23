//! Ollama-backed month-in-review narrative for the Ledger view.

use crate::assistant::ollama::{ChatMessage, ChatRole, OllamaClient, StreamChunk};
use manor_core::ledger::{budget::MonthlySummary, contract::RenewalAlert};
use tokio::sync::mpsc;

pub const REVIEW_ENDPOINT: &str = "http://127.0.0.1:11434";

pub fn build_prompt(
    year: i32,
    month: u32,
    summary: &MonthlySummary,
    renewals: &[RenewalAlert],
) -> String {
    let month_name = month_name(month);
    let mut s = format!(
        "You are a calm personal finance assistant. The user's spending for {month_name} {year}:\n\n\
         Total in: £{:.2}\n\
         Total out: £{:.2}\n\
         Net: £{:.2}\n\n\
         By category:\n",
        summary.total_in_pence as f64 / 100.0,
        summary.total_out_pence as f64 / 100.0,
        (summary.total_in_pence - summary.total_out_pence) as f64 / 100.0,
    );
    for c in &summary.by_category {
        let spent = c.spent_pence as f64 / 100.0;
        if let Some(bp) = c.budget_pence {
            let budget = bp as f64 / 100.0;
            let diff = (c.spent_pence - bp).abs() as f64 / 100.0;
            let status = if c.spent_pence > bp { "over" } else { "under" };
            s.push_str(&format!(
                "  - {} {}: £{:.2} spent, budget £{:.2}, {status} by £{:.2}\n",
                c.category_emoji, c.category_name, spent, budget, diff
            ));
        } else {
            s.push_str(&format!(
                "  - {} {}: £{:.2} spent\n",
                c.category_emoji, c.category_name, spent
            ));
        }
    }
    if !renewals.is_empty() {
        s.push_str("\nUpcoming contract renewals: ");
        let list: Vec<String> = renewals
            .iter()
            .map(|r| format!("{} in {} days", r.provider, r.days_remaining))
            .collect();
        s.push_str(&list.join(", "));
        s.push('\n');
    }
    s.push_str("\nWrite 2-3 sentences summarising what happened this month in plain English. \
                Be specific about notable categories. Do not give financial advice. No bullet points.");
    s
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

/// Run the review stream. Emits Token chunks on `out`; final Done is the caller's job.
/// `model` is the Ollama tag to request — caller resolves it from the `ai.default_model`
/// setting (see `assistant::ollama::resolve_model`).
pub async fn stream_review(prompt: String, model: String, out: mpsc::Sender<StreamChunk>) {
    let client = OllamaClient::new(REVIEW_ENDPOINT, &model);
    let msgs = vec![ChatMessage {
        role: ChatRole::User,
        content: prompt,
    }];
    let _ = client.chat(&msgs, &[], &out).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::ledger::budget::CategorySpend;

    fn fixture_summary() -> MonthlySummary {
        MonthlySummary {
            total_in_pence: 320000,
            total_out_pence: 125000,
            by_category: vec![
                CategorySpend {
                    category_id: 1,
                    category_name: "Groceries".into(),
                    category_emoji: "🛒".into(),
                    spent_pence: 45000,
                    budget_pence: Some(40000),
                },
                CategorySpend {
                    category_id: 2,
                    category_name: "Eating Out".into(),
                    category_emoji: "🍕".into(),
                    spent_pence: 12000,
                    budget_pence: None,
                },
            ],
        }
    }

    #[test]
    fn prompt_includes_totals_and_categories() {
        let p = build_prompt(2026, 4, &fixture_summary(), &[]);
        assert!(p.contains("April 2026"));
        assert!(p.contains("Total in: £3200.00"));
        assert!(p.contains("Total out: £1250.00"));
        assert!(p.contains("Net: £1950.00"));
        assert!(p.contains("🛒 Groceries: £450.00 spent, budget £400.00, over by £50.00"));
        assert!(p.contains("🍕 Eating Out: £120.00 spent"));
    }

    #[test]
    fn prompt_appends_renewals_when_present() {
        let renewals = vec![RenewalAlert {
            contract_id: 1,
            provider: "O2".into(),
            kind: "phone".into(),
            term_end: 0,
            days_remaining: 14,
            exit_fee_pence: None,
            severity: "amber".into(),
        }];
        let p = build_prompt(2026, 4, &fixture_summary(), &renewals);
        assert!(p.contains("Upcoming contract renewals: O2 in 14 days"));
    }
}
