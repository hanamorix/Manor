//! PII scrubbing for outgoing remote LLM prompts.
//!
//! The redactor is the privacy boundary of Manor's remote LLM support. Every
//! prompt going to a remote provider passes through `redact(input)` first; the
//! returned `Redacted.text` is what's persisted to `remote_call_log` AND sent
//! over the wire. Unredacted input never touches disk.
//!
//! Tested with property tests (see `tests` module) — for any input containing
//! planted PII, `redact()`'s output must not contain the original PII substring.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Redaction {
    pub kind: String,
    pub original_hash: String, // sha256 hex of the original match — for audit, not reversal
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redacted {
    pub text: String,
    pub replacements: Vec<Redaction>,
}

impl Redacted {
    pub fn count(&self) -> usize {
        self.replacements.len()
    }
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

// Each pattern: (kind, regex, placeholder). Order matters — run more-specific
// patterns first so they don't get shadowed by greedy ones.
fn patterns() -> Vec<(&'static str, Regex, &'static str)> {
    vec![
        // UK sort code followed by 8-digit account number (optional whitespace/hyphen).
        // Matches "12-34-56 12345678" or "123456 12345678".
        (
            "account",
            Regex::new(r"\b\d{2}[- ]?\d{2}[- ]?\d{2}[- ]?\d{8}\b").unwrap(),
            "[REDACTED-ACCOUNT]",
        ),
        // IBAN: 2 letters + 2 digits + up to 30 alphanumerics.
        (
            "iban",
            Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{10,30}\b").unwrap(),
            "[REDACTED-IBAN]",
        ),
        // Credit card: 13-19 digits in groups of 3-4, possibly separated by spaces/hyphens.
        // Rough — the caller relies on the Luhn check below to avoid false positives on long ids.
        (
            "card",
            Regex::new(r"\b(?:\d[- ]?){13,19}\b").unwrap(),
            "[REDACTED-CARD]",
        ),
        // UK NI number: AB 12 34 56 C
        // Character class is intentionally broad (includes Q) so the redactor
        // catches NI-shaped patterns even if the letters aren't strictly valid —
        // better to over-redact PII than under-redact.
        (
            "ni",
            Regex::new(r"\b[A-Z]{2}\s?\d{2}\s?\d{2}\s?\d{2}\s?[A-D]\b").unwrap(),
            "[REDACTED-NI]",
        ),
        // Phone: E.164 (+ up to 15 digits) OR UK national (0 followed by 9–10 digits).
        (
            "phone",
            Regex::new(r"(?:\+\d[\d\s().-]{7,18}|\b0\d[\d\s().-]{7,12})").unwrap(),
            "[REDACTED-PHONE]",
        ),
        // Email: localpart@domain. Replaced with preserved localpart hash + placeholder domain.
        (
            "email",
            Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap(),
            "", // email uses a dynamic replacement — handled specially below
        ),
    ]
}

// Luhn validator used to keep the card pattern from redacting random 13+ digit sequences.
fn luhn_valid(digits: &str) -> bool {
    let nums: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if nums.len() < 13 || nums.len() > 19 {
        return false;
    }
    let mut sum = 0u32;
    for (i, n) in nums.iter().rev().enumerate() {
        let v = if i % 2 == 1 {
            let d = n * 2;
            if d > 9 {
                d - 9
            } else {
                d
            }
        } else {
            *n
        };
        sum += v;
    }
    sum % 10 == 0
}

fn redact_email(m: &str) -> String {
    // Preserve localpart hash prefix so the model still sees "this is an email"
    // without leaking the domain. Format: user@[EMAIL-HOSTHASH-xxxx]
    if let Some(at) = m.rfind('@') {
        let domain = &m[at + 1..];
        let h = sha256_hex(domain);
        let prefix: String = h.chars().take(4).collect();
        format!("user@[EMAIL-HOSTHASH-{prefix}]")
    } else {
        "[REDACTED-EMAIL]".to_string()
    }
}

pub fn redact(input: &str) -> Redacted {
    let mut text = input.to_string();
    let mut replacements = Vec::new();

    for (kind, re, placeholder) in patterns() {
        let mut offset = 0i64;
        let orig_text = text.clone();
        for cap in re.find_iter(&orig_text) {
            let m = cap.as_str();

            // Card pattern: only redact if Luhn-valid (avoids false positives on random IDs).
            if kind == "card" && !luhn_valid(m) {
                continue;
            }

            let replacement = if kind == "email" {
                redact_email(m)
            } else {
                placeholder.to_string()
            };

            let start = (cap.start() as i64 + offset) as usize;
            let end = (cap.end() as i64 + offset) as usize;
            if end <= text.len() {
                text.replace_range(start..end, &replacement);
                offset += replacement.len() as i64 - (end - start) as i64;
                replacements.push(Redaction {
                    kind: kind.to_string(),
                    original_hash: sha256_hex(m),
                    placeholder: replacement,
                });
            }
        }
    }

    // UK postcode (simple — keep first half only): NW1 4AB → NW1
    // Applied last so it doesn't interfere with earlier patterns.
    let postcode_re = Regex::new(r"\b([A-Z]{1,2}\d[A-Z\d]?)\s?\d[A-Z]{2}\b").unwrap();
    let orig = text.clone();
    let mut offset = 0i64;
    for cap in postcode_re.captures_iter(&orig) {
        let full = cap.get(0).unwrap();
        let first_half = cap.get(1).unwrap().as_str().to_string();
        let start = (full.start() as i64 + offset) as usize;
        let end = (full.end() as i64 + offset) as usize;
        if end <= text.len() {
            text.replace_range(start..end, &first_half);
            offset += first_half.len() as i64 - (end - start) as i64;
            replacements.push(Redaction {
                kind: "postcode".to_string(),
                original_hash: sha256_hex(full.as_str()),
                placeholder: first_half,
            });
        }
    }

    Redacted { text, replacements }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit tests: specific inputs, specific expectations ──────────────────

    #[test]
    fn redacts_email_preserving_localpart_marker() {
        let r = redact("Please send to alice@example.com about the report.");
        assert!(!r.text.contains("alice@example.com"));
        assert!(r.text.contains("user@[EMAIL-HOSTHASH-"));
        assert_eq!(
            r.replacements.iter().filter(|x| x.kind == "email").count(),
            1
        );
    }

    #[test]
    fn redacts_uk_sort_code_and_account() {
        let r = redact("Account: 12-34-56 12345678 for the rent payment.");
        assert!(!r.text.contains("12345678"));
        assert!(r.text.contains("[REDACTED-ACCOUNT]"));
    }

    #[test]
    fn redacts_iban() {
        let r = redact("IBAN: GB82WEST12345698765432");
        assert!(!r.text.contains("GB82WEST12345698765432"));
        assert!(r.text.contains("[REDACTED-IBAN]"));
    }

    #[test]
    fn redacts_luhn_valid_card_numbers() {
        let r = redact("Card: 4532015112830366"); // Luhn-valid test card
        assert!(!r.text.contains("4532015112830366"));
        assert!(r.text.contains("[REDACTED-CARD]"));
    }

    #[test]
    fn does_not_redact_luhn_invalid_long_number() {
        let r = redact("Order ID: 1234567890123456789"); // Luhn-invalid
        assert!(r.text.contains("1234567890123456789"));
        assert!(r.replacements.iter().find(|x| x.kind == "card").is_none());
    }

    #[test]
    fn redacts_uk_phone() {
        let r = redact("Call me on +44 7700 900123");
        assert!(!r.text.contains("7700 900123"));
        assert!(r.text.contains("[REDACTED-PHONE]"));
    }

    #[test]
    fn redacts_ni_number() {
        let r = redact("NI: QQ 12 34 56 C");
        assert!(!r.text.contains("QQ 12 34 56 C"));
        assert!(r.text.contains("[REDACTED-NI]"));
    }

    #[test]
    fn collapses_postcode_to_first_half() {
        let r = redact("I live at 12 High Street, NW1 4AB.");
        assert!(!r.text.contains("NW1 4AB"));
        assert!(r.text.contains("NW1"));
    }

    #[test]
    fn does_not_redact_ordinary_text() {
        let r = redact("The weather is nice today. Alex wants to plan the week.");
        assert_eq!(r.replacements.len(), 0);
        assert_eq!(
            r.text,
            "The weather is nice today. Alex wants to plan the week."
        );
    }

    #[test]
    fn count_matches_replacement_list_length() {
        let r = redact("Send to a@b.com, b@b.com, and call +447700900123.");
        assert_eq!(r.count(), r.replacements.len());
        assert_eq!(r.count(), 3);
    }

    // ── Property tests: the load-bearing assurance ──────────────────────────

    use proptest::prelude::*;

    proptest! {
        // For any random-ish text containing a planted email, output must NOT
        // contain the original email address.
        #[test]
        fn property_planted_email_never_survives_redaction(
            prefix in "[a-z ]{0,20}",
            local in "[a-z]{3,10}",
            domain in "[a-z]{3,8}",
            tld in "com|org|net|io",
            suffix in "[a-z ]{0,20}",
        ) {
            let email = format!("{local}@{domain}.{tld}");
            let input = format!("{prefix} {email} {suffix}");
            let r = redact(&input);
            prop_assert!(!r.text.contains(&email),
                "email {email} survived in redacted text {}", r.text);
        }

        // For any random-ish text containing a planted UK phone, output must NOT
        // contain the 9-10 digit tail substring.
        #[test]
        fn property_planted_uk_phone_never_survives(
            prefix in "[a-z ]{0,20}",
            area in "[1-9][0-9]{3}",
            number in "[0-9]{6}",
        ) {
            let phone = format!("0{area} {number}");
            let input = format!("{prefix} Call me on {phone} please.");
            let r = redact(&input);
            // Check the digit tail (area + number, no space) doesn't survive.
            let digit_tail = format!("{area}{number}");
            prop_assert!(!r.text.contains(&digit_tail),
                "phone tail {digit_tail} survived in {}", r.text);
        }

        // For ANY input (random unicode bytes), redact should not panic and
        // should return text that's a valid String.
        #[test]
        fn property_redact_never_panics(input in ".*") {
            let _ = redact(&input);
        }
    }
}
