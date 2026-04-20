//! LLM-based schedule extraction (L4e).

use super::ExtractedSchedule;
use crate::recipe::import::{extract_json_array_block_public, LlmClient};
use anyhow::Result;

const PROMPT_PREFIX: &str = "\
You extract structured maintenance-schedule data from an appliance, vehicle, or \
fixture owner's manual. Output a JSON array with zero or more schedule objects:

[
  {
    \"task\": str,
    \"interval_months\": int (1..240),
    \"notes\": str,
    \"rationale\": str
  }
]

Requirements:
- task: a short imperative label like \"Annual service\" or \"Replace water filter\".
- interval_months: integer. Convert \"every 6 months\" to 6, \"yearly\" to 12, \
\"every 2 years\" to 24, etc. Ignore conditional intervals (e.g. \"when light blinks\").
- notes: short extra context from the manual, or empty string.
- rationale: one sentence citing where in the manual this came from \
(e.g. \"Section 7.2 recommends annual service.\").
- If no maintenance schedules are listed, output [].
- Output ONLY the JSON array. No prose before or after.

Manual text:
";

#[derive(serde::Deserialize)]
struct LlmSchedule {
    task: String,
    interval_months: i32,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    rationale: String,
}

pub async fn extract_schedules_via_llm(
    manual_text: &str,
    client: &dyn LlmClient,
) -> Result<Vec<ExtractedSchedule>> {
    let prompt = format!("{}{}", PROMPT_PREFIX, manual_text);
    let first = client.complete(&prompt).await?;

    let parsed: Result<Vec<LlmSchedule>, _> = extract_json_array_block_public(&first);
    let items = match parsed {
        Ok(v) => v,
        Err(_) => {
            let retry = format!(
                "{}\n\n(Previous response was not valid JSON. Output ONLY the JSON array.)",
                prompt
            );
            let second = client.complete(&retry).await?;
            extract_json_array_block_public::<Vec<LlmSchedule>>(&second)
                .map_err(|e| anyhow::anyhow!("failed to parse LLM JSON after retry: {}", e))?
        }
    };

    Ok(items
        .into_iter()
        .filter(|s| {
            s.interval_months >= 1 && s.interval_months <= 240 && !s.task.trim().is_empty()
        })
        .map(|s| ExtractedSchedule {
            task: s.task,
            interval_months: s.interval_months,
            notes: s.notes,
            rationale: s.rationale,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Queue-based stub: each `complete()` call dequeues the next response.
    struct StubLlmClient {
        responses: Mutex<Vec<Result<String>>>,
    }

    impl StubLlmClient {
        fn with(responses: Vec<Result<String>>) -> Self {
            Self {
                responses: Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl LlmClient for StubLlmClient {
        async fn complete(&self, _prompt: &str) -> Result<String> {
            let mut q = self.responses.lock().unwrap();
            if q.is_empty() {
                return Err(anyhow::anyhow!("stub exhausted"));
            }
            match q.remove(0) {
                Ok(s) => Ok(s),
                Err(e) => Err(e),
            }
        }
    }

    #[tokio::test]
    async fn extract_schedules_parses_valid_array() {
        let client = StubLlmClient::with(vec![Ok(r#"[
            {"task":"Annual service","interval_months":12,"notes":"","rationale":"Section 7.2."}
        ]"#
        .to_string())]);
        let out = extract_schedules_via_llm("manual text", &client)
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Annual service");
        assert_eq!(out[0].interval_months, 12);
        assert_eq!(out[0].rationale, "Section 7.2.");
    }

    #[tokio::test]
    async fn extract_schedules_retries_on_bad_json_then_succeeds() {
        let client = StubLlmClient::with(vec![
            Ok("Here's your JSON: [broken".to_string()),
            Ok(r#"[{"task":"Retry","interval_months":6,"notes":"","rationale":""}]"#.to_string()),
        ]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Retry");
    }

    #[tokio::test]
    async fn extract_schedules_returns_empty_on_empty_array() {
        let client = StubLlmClient::with(vec![Ok("[]".to_string())]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn extract_schedules_filters_invalid_intervals() {
        let client = StubLlmClient::with(vec![Ok(r#"[
            {"task":"Zero interval","interval_months":0,"notes":"","rationale":""},
            {"task":"Oversized","interval_months":300,"notes":"","rationale":""},
            {"task":"Valid","interval_months":12,"notes":"","rationale":""}
        ]"#
        .to_string())]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Valid");
    }

    #[tokio::test]
    async fn extract_schedules_filters_empty_tasks() {
        let client = StubLlmClient::with(vec![Ok(r#"[
            {"task":"","interval_months":12,"notes":"","rationale":""},
            {"task":"  ","interval_months":6,"notes":"","rationale":""},
            {"task":"Good","interval_months":12,"notes":"","rationale":""}
        ]"#
        .to_string())]);
        let out = extract_schedules_via_llm("manual", &client).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].task, "Good");
    }

    #[tokio::test]
    async fn extract_schedules_errors_on_repeated_parse_failure() {
        let client = StubLlmClient::with(vec![
            Ok("not json".to_string()),
            Ok("still not json".to_string()),
        ]);
        let err = extract_schedules_via_llm("manual", &client)
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("failed to parse LLM JSON after retry"));
    }
}
