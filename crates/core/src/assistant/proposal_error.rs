//! Typed error + result types for the proposal apply pipeline.
//!
//! Phase 1.B foundation for v0.2 Hands. `ApplyError` is the canonical error
//! shape returned from `proposal_registry::approve` (Task 1.C onward) and
//! surfaced to the frontend via the Tauri command boundary (Task 1.F).
//!
//! Both `ApplyError` and `Applied` derive `Serialize` for the IPC boundary;
//! `ApplyError` additionally derives `Deserialize` so per-item bundle errors
//! can round-trip through `proposal.apply_errors_json` (Task 1.G).
//!
//! ## Serialisation shape (load-bearing for the TS frontend)
//!
//! `ApplyError` uses serde's *adjacently-tagged* representation
//! (`#[serde(tag = "type", content = "value")]`). Each variant lands on the
//! wire as:
//!
//! ```json
//! { "type": "StaleReference", "value": { "entity": "asset", "id": "x" } }
//! { "type": "InvalidArg", "value": { "field": "...", "reason": "..." } }
//! { "type": "Conflict", "value": "row was deleted" }
//! { "type": "Network", "value": "503 from server" }
//! { "type": "UnknownKind", "value": "weird_kind" }
//! { "type": "Internal", "value": "..." }
//! ```
//!
//! Adjacent tagging is the only representation serde supports uniformly for
//! enums that mix tuple and struct variants. The frontend can
//! `switch (err.type)` and have `err.value` narrowed into the right shape.
//! Externally-tagged (`{"StaleReference": {...}}`) and internally-tagged are
//! both rejected by serde for tuple variants, so adjacent tagging is the
//! pragmatic choice that gives us a consistent TS discriminated union.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::proposal::Status;

/// Structured error returned when applying a proposal fails (or partially
/// fails). Spec §4.4.
///
/// `entity` and `field` are `String` (not `&'static str`) so the type can be
/// symmetrically deserialised from JSON — `serde` cannot synthesise a
/// `&'static str` from owned input. The allocation cost is one `String` per
/// error site, which is negligible for an error-path type.
#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
pub enum ApplyError {
    #[error("referenced {entity} {id} no longer exists")]
    StaleReference { entity: String, id: String },
    #[error("invalid argument: {field} — {reason}")]
    InvalidArg { field: String, reason: String },
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("CalDAV: {0}")]
    Network(String),
    #[error("unknown proposal kind: {0}")]
    UnknownKind(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// Result of a successful (or partially-successful) `approve` call.
///
/// `errors` is empty when `status == Status::Applied`. When at least one
/// item failed but others succeeded, `status == Status::PartiallyApplied`
/// and `errors` lists what went wrong.
#[derive(Debug, Clone, Serialize)]
pub struct Applied {
    pub proposal_id: i64,
    pub status: Status,
    pub items_applied: usize,
    pub items_failed: usize,
    pub errors: Vec<ApplyError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_json(err: &ApplyError) -> serde_json::Value {
        serde_json::from_str(&serde_json::to_string(err).unwrap()).unwrap()
    }

    #[test]
    fn stale_reference_display_matches_spec() {
        let err = ApplyError::StaleReference {
            entity: "asset".into(),
            id: "x".into(),
        };
        assert_eq!(err.to_string(), "referenced asset x no longer exists");
    }

    #[test]
    fn invalid_arg_display_matches_spec() {
        let err = ApplyError::InvalidArg {
            field: "interval_months".into(),
            reason: "must be positive".into(),
        };
        assert_eq!(
            err.to_string(),
            "invalid argument: interval_months — must be positive"
        );
    }

    #[test]
    fn conflict_display_matches_spec() {
        let err = ApplyError::Conflict("row was already updated".into());
        assert_eq!(err.to_string(), "conflict: row was already updated");
    }

    #[test]
    fn network_display_matches_spec() {
        let err = ApplyError::Network("503 from server".into());
        assert_eq!(err.to_string(), "CalDAV: 503 from server");
    }

    #[test]
    fn unknown_kind_display_matches_spec() {
        let err = ApplyError::UnknownKind("foo_bar".into());
        assert_eq!(err.to_string(), "unknown proposal kind: foo_bar");
    }

    #[test]
    fn internal_display_matches_spec() {
        let err = ApplyError::Internal("db locked".into());
        assert_eq!(err.to_string(), "internal: db locked");
    }

    #[test]
    fn stale_reference_serializes_adjacently_tagged() {
        let err = ApplyError::StaleReference {
            entity: "asset".into(),
            id: "x".into(),
        };
        let json = to_json(&err);
        assert_eq!(json["type"], "StaleReference");
        assert_eq!(json["value"]["entity"], "asset");
        assert_eq!(json["value"]["id"], "x");
    }

    #[test]
    fn invalid_arg_serializes_adjacently_tagged() {
        let err = ApplyError::InvalidArg {
            field: "interval_months".into(),
            reason: "must be positive".into(),
        };
        let json = to_json(&err);
        assert_eq!(json["type"], "InvalidArg");
        assert_eq!(json["value"]["field"], "interval_months");
        assert_eq!(json["value"]["reason"], "must be positive");
    }

    #[test]
    fn conflict_serializes_with_string_value() {
        let err = ApplyError::Conflict("row was deleted".into());
        let json = to_json(&err);
        assert_eq!(json["type"], "Conflict");
        assert_eq!(json["value"], "row was deleted");
    }

    #[test]
    fn network_serializes_with_string_value() {
        let err = ApplyError::Network("503".into());
        let json = to_json(&err);
        assert_eq!(json["type"], "Network");
        assert_eq!(json["value"], "503");
    }

    #[test]
    fn unknown_kind_serializes_with_string_value() {
        let err = ApplyError::UnknownKind("weird_kind".into());
        let json = to_json(&err);
        assert_eq!(json["type"], "UnknownKind");
        assert_eq!(json["value"], "weird_kind");
    }

    #[test]
    fn internal_serializes_with_string_value() {
        let err = ApplyError::Internal("oops".into());
        let json = to_json(&err);
        assert_eq!(json["type"], "Internal");
        assert_eq!(json["value"], "oops");
    }

    #[test]
    fn applied_serializes_with_all_fields() {
        let applied = Applied {
            proposal_id: 42,
            status: Status::Applied,
            items_applied: 3,
            items_failed: 0,
            errors: vec![],
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&applied).unwrap()).unwrap();
        assert_eq!(json["proposal_id"], 42);
        assert_eq!(json["status"], "applied");
        assert_eq!(json["items_applied"], 3);
        assert_eq!(json["items_failed"], 0);
        assert!(json["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn applied_partially_carries_errors() {
        let applied = Applied {
            proposal_id: 7,
            status: Status::PartiallyApplied,
            items_applied: 2,
            items_failed: 1,
            errors: vec![ApplyError::StaleReference {
                entity: "asset".into(),
                id: "missing".into(),
            }],
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&applied).unwrap()).unwrap();
        assert_eq!(json["status"], "partially_applied");
        assert_eq!(json["items_applied"], 2);
        assert_eq!(json["items_failed"], 1);
        let errors = json["errors"].as_array().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["type"], "StaleReference");
        assert_eq!(errors[0]["value"]["entity"], "asset");
        assert_eq!(errors[0]["value"]["id"], "missing");
    }

    /// Round-trip every variant through `serde_json` so `read_apply_errors`
    /// (Task 1.G) can recover the same Display string we serialised.
    #[test]
    fn apply_error_round_trips_through_json_for_every_variant() {
        let cases = vec![
            ApplyError::StaleReference {
                entity: "asset".into(),
                id: "x".into(),
            },
            ApplyError::InvalidArg {
                field: "interval_months".into(),
                reason: "must be positive".into(),
            },
            ApplyError::Conflict("row was deleted".into()),
            ApplyError::Network("503 from server".into()),
            ApplyError::UnknownKind("weird_kind".into()),
            ApplyError::Internal("db locked".into()),
        ];
        for err in cases {
            let s = serde_json::to_string(&err).unwrap();
            let back: ApplyError = serde_json::from_str(&s).unwrap();
            assert_eq!(
                format!("{}", err),
                format!("{}", back),
                "round-trip Display mismatch for {err:?}"
            );
        }
    }
}
