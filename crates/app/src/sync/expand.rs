//! Expand a parent VEVENT's RRULE into individual NewEvent rows within a window.

use anyhow::Result;
use chrono::{DateTime, Utc};
use manor_core::assistant::event::NewEvent;
use rrule::{RRuleSet, Tz as RruleTz};
use std::str::FromStr;

use crate::sync::ical::ParsedEvent;

/// Expand `ev` over the [window_start, window_end) range.
///
/// Non-recurring events (rrule=None) yield exactly one NewEvent with `external_id = uid`.
/// Recurring events yield one NewEvent per occurrence within the window, with
/// `external_id = "{uid}::{RFC3339-start}"` for deterministic re-sync.
pub fn expand(
    ev: &ParsedEvent,
    account_id: i64,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    event_url: &str,
) -> Result<Vec<NewEvent>> {
    let duration = ev.end_at - ev.start_at;

    // Non-recurring: pass through if it falls inside the window.
    let Some(rrule_str) = &ev.rrule else {
        if ev.start_at >= window_start.timestamp() && ev.start_at < window_end.timestamp() {
            return Ok(vec![NewEvent {
                calendar_account_id: account_id,
                external_id: ev.uid.clone(),
                title: ev.summary.clone(),
                start_at: ev.start_at,
                end_at: ev.end_at,
                event_url: Some(event_url.to_string()),
                etag: None,
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: false,
                parent_event_url: None,
                occurrence_dtstart: None,
            }]);
        }
        return Ok(vec![]);
    };

    // Recurring: build RRuleSet and enumerate.
    let parent_start_utc = DateTime::<Utc>::from_timestamp(ev.start_at, 0)
        .ok_or_else(|| anyhow::anyhow!("bad parent start_at"))?;

    // rrule 0.13 expects DTSTART in its own format. Compose the string it wants.
    let dtstart_line = format!("DTSTART:{}\n", parent_start_utc.format("%Y%m%dT%H%M%SZ"));
    let rule_block = format!("{dtstart_line}RRULE:{rrule_str}");
    let mut rset = RRuleSet::from_str(&rule_block)?;

    let window_start_rrule = window_start.with_timezone(&RruleTz::UTC);
    let window_end_rrule = window_end.with_timezone(&RruleTz::UTC);

    // Use after/before/all chain to get occurrences within the window.
    rset = rset.after(window_start_rrule).before(window_end_rrule);
    let result = rset.all(10_000); // Reasonable upper limit for a single sync window

    let exdate_set: std::collections::HashSet<String> = ev.exdates.iter().cloned().collect();

    let out = result
        .dates
        .into_iter()
        .filter_map(|occ| {
            let occ_utc = occ.with_timezone(&Utc);
            let rfc = occ_utc.to_rfc3339();
            if exdate_set.contains(&rfc) {
                return None;
            }
            let start = occ_utc.timestamp();
            let occ_dtstart = occ_utc.format("%Y%m%dT%H%M%SZ").to_string();
            Some(NewEvent {
                calendar_account_id: account_id,
                external_id: format!("{}::{}", ev.uid, rfc),
                title: ev.summary.clone(),
                start_at: start,
                end_at: start + duration,
                event_url: None,
                etag: None,
                description: None,
                location: None,
                all_day: false,
                is_recurring_occurrence: true,
                parent_event_url: Some(event_url.to_string()),
                occurrence_dtstart: Some(occ_dtstart),
            })
        })
        .collect();

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_weekly() -> ParsedEvent {
        // Weekly event every Wednesday starting 2026-04-15 09:30 UTC
        ParsedEvent {
            uid: "weekly-1".into(),
            summary: "Standup".into(),
            start_at: Utc
                .with_ymd_and_hms(2026, 4, 15, 9, 30, 0)
                .unwrap()
                .timestamp(),
            end_at: Utc
                .with_ymd_and_hms(2026, 4, 15, 10, 0, 0)
                .unwrap()
                .timestamp(),
            rrule: Some("FREQ=WEEKLY;BYDAY=WE".into()),
            exdates: vec![],
            dtstart_raw: "20260415T093000Z".into(),
        }
    }

    #[test]
    fn non_recurring_event_yields_one_newevent_with_uid_as_external_id() {
        let ev = ParsedEvent {
            uid: "once".into(),
            summary: "Boiler".into(),
            start_at: Utc
                .with_ymd_and_hms(2026, 4, 15, 10, 0, 0)
                .unwrap()
                .timestamp(),
            end_at: Utc
                .with_ymd_and_hms(2026, 4, 15, 11, 0, 0)
                .unwrap()
                .timestamp(),
            rrule: None,
            exdates: vec![],
            dtstart_raw: "20260415T100000Z".into(),
        };
        let start = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 20, 0, 0, 0).unwrap();
        let out = expand(&ev, 1, start, end, "https://cal.example.com/event.ics").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].external_id, "once");
    }

    #[test]
    fn expands_weekly_rrule_in_window() {
        let ev = sample_weekly();
        let start = Utc.with_ymd_and_hms(2026, 4, 8, 0, 0, 0).unwrap(); // today-7
        let end = Utc.with_ymd_and_hms(2026, 4, 29, 0, 0, 0).unwrap(); // today+14
        let out = expand(&ev, 1, start, end, "https://cal.example.com/event.ics").unwrap();
        // Wed 2026-04-15 and Wed 2026-04-22 — two occurrences
        assert_eq!(out.len(), 2);
        assert!(out[0].external_id.starts_with("weekly-1::"));
        assert!(out[0].external_id.contains("2026-04-15T09:30:00"));
    }

    #[test]
    fn applies_exdate_exclusions() {
        let mut ev = sample_weekly();
        ev.exdates = vec!["2026-04-22T09:30:00+00:00".into()];
        let start = Utc.with_ymd_and_hms(2026, 4, 8, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 29, 0, 0, 0).unwrap();
        let out = expand(&ev, 1, start, end, "https://cal.example.com/event.ics").unwrap();
        // Only the 2026-04-15 occurrence remains; 2026-04-22 is excluded.
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn deterministic_external_id_format() {
        let ev = sample_weekly();
        let start = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 17, 0, 0, 0).unwrap();
        let a = expand(&ev, 1, start, end, "https://cal.example.com/event.ics").unwrap();
        let b = expand(&ev, 1, start, end, "https://cal.example.com/event.ics").unwrap();
        assert_eq!(a[0].external_id, b[0].external_id);
    }

    #[test]
    fn non_recurring_event_gets_event_url() {
        let ev = ParsedEvent {
            uid: "once".into(),
            summary: "Boiler".into(),
            start_at: Utc
                .with_ymd_and_hms(2026, 4, 15, 10, 0, 0)
                .unwrap()
                .timestamp(),
            end_at: Utc
                .with_ymd_and_hms(2026, 4, 15, 11, 0, 0)
                .unwrap()
                .timestamp(),
            rrule: None,
            exdates: vec![],
            dtstart_raw: "20260415T100000Z".into(),
        };
        let start = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 20, 0, 0, 0).unwrap();
        let out = expand(&ev, 1, start, end, "https://cal.example.com/home/event.ics").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].event_url.as_deref(),
            Some("https://cal.example.com/home/event.ics")
        );
        assert!(!out[0].is_recurring_occurrence);
    }

    #[test]
    fn recurring_occurrences_get_parent_url_and_flag() {
        let ev = sample_weekly();
        let start = Utc.with_ymd_and_hms(2026, 4, 8, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 29, 0, 0, 0).unwrap();
        let out = expand(
            &ev,
            1,
            start,
            end,
            "https://cal.example.com/home/standup.ics",
        )
        .unwrap();
        assert!(!out.is_empty());
        assert!(out[0].is_recurring_occurrence);
        assert_eq!(
            out[0].parent_event_url.as_deref(),
            Some("https://cal.example.com/home/standup.ics")
        );
        assert!(out[0].occurrence_dtstart.is_some());
    }
}
