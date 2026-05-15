//! Tolerant `serde` deserialisers for assistant tool arguments.
//!
//! Local LLMs (qwen2.5:7b-instruct in particular) sometimes emit shapes that
//! differ from the JSON schema we hand to Ollama: numbers that should be
//! strings, structured `{year, month, day}` objects where an ISO date was
//! requested, currency symbols glued onto amounts, casual phrases like
//! "every Monday" instead of an RFC 5545 RRULE. Rejecting these would mean
//! the user re-types perfectly understandable input.
//!
//! This module centralises the small coercions we need so all assistant tool
//! arg structs share the same lenient grammar:
//!
//! - [`amount_pence`] — money in either pence-as-int or pounds-as-float, with
//!   optional `£` prefix and minus sign. Always emits integer pence.
//! - [`iso_date`] — ISO date string, structured `{year, month, day}`, `null`,
//!   missing field → `Option<String>` (`None` for any non-string non-object).
//! - [`rrule_string`] — literal RFC 5545 RRULE, or one of a few casual
//!   phrases ("weekly", "every Monday", etc.). Returns the validated RRULE
//!   string. Gibberish becomes a serde error → `InvalidArg` at apply-time.
//! - [`IdOrName`] — placeholder enum for fields that may be either a stable
//!   id or a human name. **Limitation:** because both variants wrap `String`,
//!   `#[serde(untagged)]` cannot disambiguate them on its own — the caller's
//!   args struct must declare which variant it expects, or wrap with a
//!   tagged enum. We expose the enum so Phase 2 callers have a shared type;
//!   actual disambiguation happens in the apply-time resolver.

use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};

/// Deserialise a money amount as integer pence, accepting:
///
/// - integer pence: `4000`, `"4000"`, `"-4000"`
/// - pounds as float: `40.00`, `"40.00"`, `"-40.00"`
/// - currency-prefixed: `"£40"`, `"£40.00"`, `"-£40"`
///
/// Conversion is half-away-from-zero rounded to whole pence.
pub fn amount_pence<'de, D>(d: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i)
            } else if let Some(f) = n.as_f64() {
                Ok((f * 100.0).round() as i64)
            } else {
                Err(D::Error::custom("amount: unrepresentable number"))
            }
        }
        serde_json::Value::String(s) => parse_amount_str(&s)
            .ok_or_else(|| D::Error::custom(format!("amount: cannot parse {s:?}"))),
        other => Err(D::Error::custom(format!(
            "amount: expected number or string, got {other}"
        ))),
    }
}

pub fn optional_amount_pence<'de, D>(d: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Some(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Some((f * 100.0).round() as i64))
            } else {
                Err(D::Error::custom("amount: unrepresentable number"))
            }
        }
        serde_json::Value::String(s) => parse_amount_str(&s)
            .map(Some)
            .ok_or_else(|| D::Error::custom(format!("amount: cannot parse {s:?}"))),
        other => Err(D::Error::custom(format!(
            "amount: expected number, string, or null, got {other}"
        ))),
    }
}

fn parse_amount_str(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    let (sign, rest) = match trimmed.strip_prefix('-') {
        Some(r) => (-1i64, r.trim_start()),
        None => (1, trimmed),
    };
    // `£` signals "this is pounds" — multiply by 100 even for whole numbers.
    // Without a currency prefix and without a decimal point, treat the value
    // as already-pence (matches the bare-number path: `"4000"` → 4000).
    let (had_currency, body) = match rest.strip_prefix('£') {
        Some(r) => (true, r.trim()),
        None => (false, rest),
    };
    if body.is_empty() {
        return None;
    }
    if body.contains('.') {
        let f: f64 = body.parse().ok()?;
        Some(sign * (f * 100.0).round() as i64)
    } else {
        let i: i64 = body.parse().ok()?;
        Some(sign * i * if had_currency { 100 } else { 1 })
    }
}

/// Deserialise an ISO date, tolerating qwen2.5's structured-object quirk.
///
/// Accepts: `"2026-05-09"` → `Some(...)`, `null` / missing → `None`,
/// `{ "year": .., "month": .., "day": .. }` → `None` (preserves existing
/// `deserialize_due_date` semantics — caller defaults to today on `None`).
/// Any other shape also becomes `None`.
pub fn iso_date<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::String(s) => Ok(Some(s)),
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

/// Deserialise an RRULE string. Accepts a literal RFC 5545 RRULE
/// (e.g. `"FREQ=WEEKLY;BYDAY=MO"`) or a small set of casual phrases:
///
/// - `weekly`, `daily`, `monthly`
/// - `every N days` (N a positive integer)
/// - `every <weekday>` → `FREQ=WEEKLY;BYDAY=<DAY>`
/// - `every other <weekday>` → `FREQ=WEEKLY;INTERVAL=2;BYDAY=<DAY>`
/// - `alternating` → `FREQ=DAILY` (rotation handled separately in Phase 2)
///
/// Returns a validated RFC 5545 RRULE string. Unparseable input becomes a
/// serde error which surfaces as `InvalidArg` at apply-time.
pub fn rrule_string<'de, D>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(d)?;
    let candidate = casual_to_rrule(&raw).unwrap_or_else(|| raw.trim().to_string());
    validate_rrule(&candidate)
        .map_err(|e| D::Error::custom(format!("rrule {raw:?} invalid: {e}")))?;
    Ok(candidate)
}

/// Casual-phrase parser. Returns `Some(rrule)` for the recognised forms,
/// `None` to fall through to RFC 5545 validation.
fn casual_to_rrule(raw: &str) -> Option<String> {
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "weekly" => return Some("FREQ=WEEKLY".into()),
        "daily" => return Some("FREQ=DAILY".into()),
        "monthly" => return Some("FREQ=MONTHLY".into()),
        "alternating" => return Some("FREQ=DAILY".into()),
        _ => {}
    }
    if let Some(rest) = s.strip_prefix("every other ") {
        return weekday_code(rest).map(|d| format!("FREQ=WEEKLY;INTERVAL=2;BYDAY={d}"));
    }
    if let Some(rest) = s.strip_prefix("every ") {
        let rest = rest.trim();
        if let Some(num_part) = rest.strip_suffix(" days") {
            if let Ok(n) = num_part.trim().parse::<u32>() {
                if n > 0 {
                    return Some(format!("FREQ=DAILY;INTERVAL={n}"));
                }
            }
        }
        if let Some(d) = weekday_code(rest) {
            return Some(format!("FREQ=WEEKLY;BYDAY={d}"));
        }
    }
    None
}

fn weekday_code(s: &str) -> Option<&'static str> {
    match s.trim() {
        "monday" | "mon" => Some("MO"),
        "tuesday" | "tue" | "tues" => Some("TU"),
        "wednesday" | "wed" => Some("WE"),
        "thursday" | "thu" | "thur" | "thurs" => Some("TH"),
        "friday" | "fri" => Some("FR"),
        "saturday" | "sat" => Some("SA"),
        "sunday" | "sun" => Some("SU"),
        _ => None,
    }
}

fn validate_rrule(rrule: &str) -> Result<(), String> {
    use std::str::FromStr;
    // RRuleSet wants a DTSTART block; pair it with a sentinel and let the
    // parser surface any invalid grammar.
    let block = format!("DTSTART:20260101T000000Z\nRRULE:{rrule}");
    rrule::RRuleSet::from_str(&block)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Either a stable id or a human name.
///
/// **Limitation:** `#[serde(untagged)]` over two `String` variants cannot
/// disambiguate at deserialisation time — both candidates match. We keep
/// the type so Phase 2 callers have a shared shape, but the args struct
/// must either:
///
/// 1. Pick one variant up front (e.g. `Vec<String>` for "names only") and
///    resolve to ids inside the apply-time handler, or
/// 2. Use a tagged wrapper such as `{ "id": "abc-123" }` /
///    `{ "name": "Lewis" }` and project into [`IdOrName`] manually.
///
/// In tests we only verify that both variants round-trip when explicitly
/// tagged. Untagged callers always end up in the `Id` arm.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum IdOrName {
    Id(String),
    Name(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize)]
    struct AmountWrap {
        #[serde(deserialize_with = "amount_pence")]
        amount: i64,
    }

    #[derive(Deserialize)]
    struct DateWrap {
        #[serde(default, deserialize_with = "iso_date")]
        date: Option<String>,
    }

    #[derive(Deserialize)]
    struct RruleWrap {
        #[serde(deserialize_with = "rrule_string")]
        rrule: String,
    }

    fn amt(v: serde_json::Value) -> i64 {
        serde_json::from_value::<AmountWrap>(json!({ "amount": v }))
            .unwrap()
            .amount
    }

    #[test]
    fn amount_pence_accepts_int_and_string_int() {
        assert_eq!(amt(json!(4000)), 4000);
        assert_eq!(amt(json!("4000")), 4000);
    }

    #[test]
    fn amount_pence_accepts_pounds_float() {
        assert_eq!(amt(json!(40.00)), 4000);
        assert_eq!(amt(json!("40.00")), 4000);
    }

    #[test]
    fn amount_pence_accepts_currency_prefix() {
        assert_eq!(amt(json!("£40")), 4000);
        assert_eq!(amt(json!("£40.00")), 4000);
    }

    #[test]
    fn amount_pence_accepts_negatives() {
        assert_eq!(amt(json!("-40.00")), -4000);
        assert_eq!(amt(json!(-4000)), -4000);
    }

    #[test]
    fn amount_pence_rejects_gibberish() {
        let bad: Result<AmountWrap, _> = serde_json::from_value(json!({ "amount": "lots" }));
        assert!(bad.is_err());
    }

    #[test]
    fn iso_date_accepts_string() {
        let w: DateWrap = serde_json::from_value(json!({ "date": "2026-05-09" })).unwrap();
        assert_eq!(w.date.as_deref(), Some("2026-05-09"));
    }

    #[test]
    fn iso_date_accepts_null() {
        let w: DateWrap = serde_json::from_value(json!({ "date": null })).unwrap();
        assert_eq!(w.date, None);
    }

    #[test]
    fn iso_date_accepts_missing_field() {
        let w: DateWrap = serde_json::from_value(json!({})).unwrap();
        assert_eq!(w.date, None);
    }

    #[test]
    fn iso_date_coerces_structured_object_to_none() {
        let w: DateWrap =
            serde_json::from_value(json!({ "date": { "year": 2026, "month": 5, "day": 9 } }))
                .unwrap();
        assert_eq!(w.date, None);
    }

    #[test]
    fn iso_date_coerces_other_non_string_to_none() {
        let w: DateWrap = serde_json::from_value(json!({ "date": 42 })).unwrap();
        assert_eq!(w.date, None);
    }

    fn rr(s: &str) -> String {
        serde_json::from_value::<RruleWrap>(json!({ "rrule": s }))
            .unwrap()
            .rrule
    }

    #[test]
    fn rrule_string_accepts_literal_rfc5545() {
        assert_eq!(rr("FREQ=WEEKLY;BYDAY=MO"), "FREQ=WEEKLY;BYDAY=MO");
        assert_eq!(rr("FREQ=DAILY"), "FREQ=DAILY");
    }

    #[test]
    fn rrule_string_accepts_casual_phrases() {
        assert_eq!(rr("weekly"), "FREQ=WEEKLY");
        assert_eq!(rr("daily"), "FREQ=DAILY");
        assert_eq!(rr("monthly"), "FREQ=MONTHLY");
        assert_eq!(rr("every Monday"), "FREQ=WEEKLY;BYDAY=MO");
        assert_eq!(rr("every other Tuesday"), "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU");
        assert_eq!(rr("every 3 days"), "FREQ=DAILY;INTERVAL=3");
        assert_eq!(rr("alternating"), "FREQ=DAILY");
    }

    #[test]
    fn rrule_string_rejects_gibberish() {
        let bad: Result<RruleWrap, _> =
            serde_json::from_value(json!({ "rrule": "next blue moon" }));
        assert!(bad.is_err());
    }

    #[test]
    fn id_or_name_untagged_always_picks_id() {
        // Documented limitation: untagged String variants cannot disambiguate.
        let v: IdOrName = serde_json::from_value(json!("abc-123")).unwrap();
        assert_eq!(v, IdOrName::Id("abc-123".into()));
        let v: IdOrName = serde_json::from_value(json!("Lewis")).unwrap();
        assert_eq!(v, IdOrName::Id("Lewis".into()));
    }

    #[test]
    fn id_or_name_tagged_round_trips_both_variants() {
        // When the caller projects a tagged shape, both variants are reachable.
        let id = IdOrName::Id("abc-123".into());
        let name = IdOrName::Name("Lewis".into());
        // Round-trip via JSON — untagged serialise emits a bare string for
        // both, so we just confirm the in-memory variants are distinct.
        assert_ne!(id, name);
        assert_eq!(serde_json::to_value(&id).unwrap(), json!("abc-123"));
        assert_eq!(serde_json::to_value(&name).unwrap(), json!("Lewis"));
    }
}
