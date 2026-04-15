//! iCal (RFC 5545) VEVENT parsing.
//!
//! We only extract what Manor actually stores: UID, DTSTART, DTEND (or DURATION),
//! SUMMARY, and — for later recurrence expansion — RRULE and EXDATE. Everything
//! else (LOCATION, DESCRIPTION, ATTENDEE, ALARM, …) is ignored.

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use ical::parser::ical::component::IcalEvent;
use ical::property::Property;
use ical::IcalParser;

/// Intermediate shape after parsing, before RRULE expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    pub uid: String,
    pub summary: String,
    /// Start instant in UTC seconds.
    pub start_at: i64,
    /// End instant in UTC seconds.
    pub end_at: i64,
    /// RRULE string as it appears in the iCal (e.g. `FREQ=WEEKLY;BYDAY=MO`), or None.
    pub rrule: Option<String>,
    /// EXDATE values as RFC3339 UTC strings (already converted), or empty.
    pub exdates: Vec<String>,
    /// The original DTSTART string (preserved — rrule crate needs the raw context).
    pub dtstart_raw: String,
}

/// Parse a single VEVENT into a ParsedEvent.
///
/// Errors out only for events missing required properties; malformed values
/// within an otherwise-intact VEVENT produce a best-effort result.
pub fn parse_vevent(ev: &IcalEvent, local_tz: Tz) -> Result<ParsedEvent> {
    let uid = prop_value(ev, "UID").ok_or_else(|| anyhow!("VEVENT missing UID"))?;
    let summary = prop_value(ev, "SUMMARY").unwrap_or_else(|| "(no title)".to_string());

    let dtstart_prop = ev
        .properties
        .iter()
        .find(|p| p.name == "DTSTART")
        .ok_or_else(|| anyhow!("VEVENT missing DTSTART"))?;
    let dtstart_raw = dtstart_prop.value.clone().unwrap_or_default();
    let start_at = parse_dt(dtstart_prop, local_tz)?;

    let end_at = if let Some(dtend_prop) = ev.properties.iter().find(|p| p.name == "DTEND") {
        parse_dt(dtend_prop, local_tz)?
    } else if let Some(dur) = prop_value(ev, "DURATION") {
        start_at + parse_duration_seconds(&dur)?
    } else {
        bail!("VEVENT {uid} missing both DTEND and DURATION");
    };

    let rrule = prop_value(ev, "RRULE");

    let exdates: Vec<String> = ev
        .properties
        .iter()
        .filter(|p| p.name == "EXDATE")
        .filter_map(|p| {
            let v = p.value.as_ref()?;
            parse_dt(p, local_tz).ok().map(|secs| {
                DateTime::<Utc>::from_timestamp(secs, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| v.clone())
            })
        })
        .collect();

    Ok(ParsedEvent {
        uid,
        summary,
        start_at,
        end_at,
        rrule,
        exdates,
        dtstart_raw,
    })
}

fn prop_value(ev: &IcalEvent, name: &str) -> Option<String> {
    ev.properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.clone())
}

/// Parse a DTSTART / DTEND / EXDATE value using its parameters (VALUE=DATE, TZID=...).
/// Returns the instant as UTC seconds.
fn parse_dt(prop: &Property, local_tz: Tz) -> Result<i64> {
    let value = prop
        .value
        .as_ref()
        .ok_or_else(|| anyhow!("{} missing value", prop.name))?;
    let params = prop.params.as_deref().unwrap_or(&[]);

    let is_date_only = params
        .iter()
        .any(|(k, vals)| k == "VALUE" && vals.iter().any(|v| v == "DATE"));
    let tzid = params.iter().find_map(|(k, vals)| {
        if k == "TZID" {
            vals.first().cloned()
        } else {
            None
        }
    });

    if is_date_only {
        // YYYYMMDD — all-day, anchored to system-local midnight.
        let d = NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|e| anyhow!("bad DATE value {value}: {e}"))?;
        let naive = d.and_hms_opt(0, 0, 0).unwrap();
        let local_dt = local_tz
            .from_local_datetime(&naive)
            .single()
            .ok_or_else(|| anyhow!("ambiguous local datetime for {value}"))?;
        return Ok(local_dt.with_timezone(&Utc).timestamp());
    }

    if value.ends_with('Z') {
        // YYYYMMDDTHHMMSSZ — UTC.
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
            .map_err(|e| anyhow!("bad UTC datetime {value}: {e}"))?;
        return Ok(Utc.from_utc_datetime(&naive).timestamp());
    }

    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|e| anyhow!("bad naive datetime {value}: {e}"))?;

    let tz: Tz = match tzid {
        Some(name) => name.parse().unwrap_or(chrono_tz::UTC),
        None => local_tz,
    };
    let local_dt = tz
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| anyhow!("ambiguous local datetime for {value}"))?;
    Ok(local_dt.with_timezone(&Utc).timestamp())
}

fn parse_duration_seconds(s: &str) -> Result<i64> {
    // ISO-8601-ish: P[nD]T[nH][nM][nS]. We only handle hours/minutes/days; weeks/months rare in VEVENT.
    let mut secs: i64 = 0;
    let mut num = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
            continue;
        }
        let n: i64 = if num.is_empty() {
            0
        } else {
            num.parse()
                .map_err(|_| anyhow!("bad number in duration {s}"))?
        };
        num.clear();
        match ch {
            'W' => secs += n * 7 * 86_400,
            'D' => secs += n * 86_400,
            'H' => secs += n * 3600,
            'M' => secs += n * 60,
            'S' => secs += n,
            'P' | 'T' | '+' | '-' => {}
            _ => bail!("unexpected character {ch:?} in duration {s}"),
        }
    }
    Ok(secs)
}

/// Parse a full VCALENDAR string and return all VEVENTs that parse successfully.
/// Events that fail to parse individually are logged and skipped.
pub fn parse_vcalendar(ics: &str, local_tz: Tz) -> Vec<ParsedEvent> {
    let reader = IcalParser::new(ics.as_bytes());
    let mut out = Vec::new();
    for cal_result in reader {
        let cal = match cal_result {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("skipping malformed VCALENDAR: {e}");
                continue;
            }
        };
        for ev in cal.events {
            match parse_vevent(&ev, local_tz) {
                Ok(parsed) => out.push(parsed),
                Err(e) => tracing::warn!("skipping malformed VEVENT: {e}"),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ldn() -> Tz {
        chrono_tz::Europe::London
    }

    #[test]
    fn parses_utc_dtstart() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:u1\r\nDTSTART:20260415T093000Z\r\nDTEND:20260415T103000Z\r\nSUMMARY:Boiler\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "u1");
        assert_eq!(events[0].summary, "Boiler");
        // 2026-04-15T09:30:00Z
        let expected = DateTime::parse_from_rfc3339("2026-04-15T09:30:00+00:00")
            .unwrap()
            .timestamp();
        assert_eq!(events[0].start_at, expected);
    }

    #[test]
    fn parses_tzid_dtstart_via_chrono_tz() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:u2\r\nDTSTART;TZID=Europe/London:20260415T093000\r\nDTEND;TZID=Europe/London:20260415T103000\r\nSUMMARY:Meeting\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        // 09:30 London in mid-April = UTC 08:30 (BST, +1)
        let expected = DateTime::parse_from_rfc3339("2026-04-15T08:30:00+00:00")
            .unwrap()
            .timestamp();
        assert_eq!(events[0].start_at, expected);
    }

    #[test]
    fn parses_all_day_as_midnight_local_pair() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:u3\r\nDTSTART;VALUE=DATE:20260415\r\nDTEND;VALUE=DATE:20260416\r\nSUMMARY:Birthday\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        // Midnight London = UTC 23:00 previous day (BST, +1)
        let midnight_local = DateTime::parse_from_rfc3339("2026-04-14T23:00:00+00:00")
            .unwrap()
            .timestamp();
        assert_eq!(events[0].start_at, midnight_local);
        assert_eq!(events[0].end_at - events[0].start_at, 86_400);
    }

    #[test]
    fn skips_malformed_vevent_others_survive() {
        // First VEVENT missing UID → skipped. Second is valid.
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:NoUID\r\nDTSTART:20260415T093000Z\r\nDTEND:20260415T103000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:ok\r\nDTSTART:20260415T110000Z\r\nDTEND:20260415T120000Z\r\nSUMMARY:OK\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "ok");
    }

    #[test]
    fn extracts_rrule_and_exdate() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:recurring\r\nDTSTART:20260415T093000Z\r\nDTEND:20260415T103000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=WE\r\nEXDATE:20260422T093000Z\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].rrule.as_deref(), Some("FREQ=WEEKLY;BYDAY=WE"));
        assert_eq!(events[0].exdates.len(), 1);
    }

    #[test]
    fn uses_duration_when_dtend_absent() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:dur\r\nDTSTART:20260415T093000Z\r\nDURATION:PT1H30M\r\nSUMMARY:Dur\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_vcalendar(ics, ldn());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].end_at - events[0].start_at, 5400);
    }
}
