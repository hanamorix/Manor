//! iCal generation helpers for CalDAV write operations.
//! Produces RFC 5545-compliant VCALENDAR strings.

/// Format a UTC timestamp (unix seconds) as iCal DTSTART/DTEND value.
fn fmt_utc(ts: i64) -> String {
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0).unwrap_or_default();
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Fold a long iCal line at 75 octets per RFC 5545 §3.1.
/// Lines > 75 bytes are wrapped with CRLF + SP.
fn fold_line(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut pos = 0;
    let mut first = true;
    while pos < bytes.len() {
        let limit = if first { 75 } else { 74 }; // first line 75, continuation 74 (1 for space)
                                                 // find a safe split on a char boundary
        let end = (pos + limit).min(bytes.len());
        // walk back to char boundary if needed
        let mut safe = end;
        while safe > pos && !line.is_char_boundary(safe) {
            safe -= 1;
        }
        if !first {
            out.push(' ');
        }
        out.push_str(&line[pos..safe]);
        out.push_str("\r\n");
        pos = safe;
        first = false;
    }
    out
}

/// Generate a complete VCALENDAR string for a new VEVENT.
pub fn generate_vcalendar(
    uid: &str,
    summary: &str,
    dtstart_utc: i64,
    dtend_utc: i64,
    description: Option<&str>,
    location: Option<&str>,
    all_day: bool,
) -> String {
    let (dtstart_val, dtend_val) = if all_day {
        let start_dt =
            chrono::DateTime::<chrono::Utc>::from_timestamp(dtstart_utc, 0).unwrap_or_default();
        let end_dt =
            chrono::DateTime::<chrono::Utc>::from_timestamp(dtend_utc, 0).unwrap_or_default();
        (
            format!("VALUE=DATE:{}", start_dt.format("%Y%m%d")),
            format!("VALUE=DATE:{}", end_dt.format("%Y%m%d")),
        )
    } else {
        (fmt_utc(dtstart_utc), fmt_utc(dtend_utc))
    };

    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//Manor//CalDAV Write//EN".into(),
        "BEGIN:VEVENT".into(),
        fold_line(&format!("UID:{uid}")),
        fold_line(&format!("SUMMARY:{summary}")),
    ];

    if all_day {
        lines.push(fold_line(&format!("DTSTART;{dtstart_val}")));
        lines.push(fold_line(&format!("DTEND;{dtend_val}")));
    } else {
        lines.push(fold_line(&format!("DTSTART:{dtstart_val}")));
        lines.push(fold_line(&format!("DTEND:{dtend_val}")));
    }

    if let Some(desc) = description {
        if !desc.is_empty() {
            lines.push(fold_line(&format!("DESCRIPTION:{desc}")));
        }
    }
    if let Some(loc) = location {
        if !loc.is_empty() {
            lines.push(fold_line(&format!("LOCATION:{loc}")));
        }
    }
    lines.push("END:VEVENT".into());
    lines.push("END:VCALENDAR".into());

    lines.join("\r\n") + "\r\n"
}

/// Add an EXDATE to a recurring parent event's iCal source to skip one occurrence.
/// `occurrence_dtstart_utc` is in `YYYYMMDDTHHMMSSz` format (iCal UTC notation).
pub fn add_exdate(ical: &str, occurrence_dtstart_utc: &str) -> String {
    // Insert EXDATE line immediately before END:VEVENT
    let exdate_line = format!("EXDATE:{occurrence_dtstart_utc}");
    ical.replacen("END:VEVENT", &format!("{}\r\nEND:VEVENT", exdate_line), 1)
}

/// Add a RECURRENCE-ID override VEVENT to a parent iCal (edit one occurrence).
/// The override VEVENT is inserted before END:VCALENDAR.
pub fn add_recurrence_override(
    ical: &str,
    recurrence_id_utc: &str,
    summary: &str,
    dtstart_utc: i64,
    dtend_utc: i64,
    description: Option<&str>,
    location: Option<&str>,
) -> String {
    // Extract UID from parent
    let uid = ical
        .lines()
        .find(|l| l.trim_start_matches(' ').starts_with("UID:"))
        .map(|l| l.trim_start_matches(' ').trim_start_matches("UID:").trim())
        .unwrap_or("unknown");

    let mut override_lines: Vec<String> = vec![
        "BEGIN:VEVENT".into(),
        fold_line(&format!("UID:{uid}")),
        fold_line(&format!("RECURRENCE-ID:{recurrence_id_utc}")),
        fold_line(&format!("SUMMARY:{summary}")),
        fold_line(&format!("DTSTART:{}", fmt_utc(dtstart_utc))),
        fold_line(&format!("DTEND:{}", fmt_utc(dtend_utc))),
    ];
    if let Some(desc) = description {
        if !desc.is_empty() {
            override_lines.push(fold_line(&format!("DESCRIPTION:{desc}")));
        }
    }
    if let Some(loc) = location {
        if !loc.is_empty() {
            override_lines.push(fold_line(&format!("LOCATION:{loc}")));
        }
    }
    override_lines.push("END:VEVENT".into());

    let override_block = override_lines.join("\r\n");
    ical.replacen(
        "END:VCALENDAR",
        &format!("{}\r\nEND:VCALENDAR", override_block),
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_vcalendar_basic_event() {
        let ical = generate_vcalendar(
            "test-uid-1",
            "Team Lunch",
            1_745_000_000,
            1_745_003_600,
            None,
            None,
            false,
        );
        assert!(ical.contains("BEGIN:VCALENDAR"));
        assert!(ical.contains("UID:test-uid-1"));
        assert!(ical.contains("SUMMARY:Team Lunch"));
        assert!(ical.contains("BEGIN:VEVENT"));
        assert!(ical.contains("END:VEVENT"));
        assert!(ical.contains("END:VCALENDAR"));
        assert!(ical.ends_with("\r\n"), "must end with CRLF");
    }

    #[test]
    fn generate_vcalendar_all_day_uses_value_date() {
        // 2025-04-16 midnight UTC = 1744761600
        let ical = generate_vcalendar(
            "allday-1",
            "Holiday",
            1_744_761_600,
            1_744_848_000,
            None,
            None,
            true,
        );
        assert!(ical.contains("DTSTART;VALUE=DATE:20250416"));
        assert!(
            !ical.contains("DTSTART:20250416T"),
            "must not use time-based DTSTART for all-day"
        );
    }

    #[test]
    fn generate_vcalendar_with_description_and_location() {
        let ical = generate_vcalendar(
            "ev-desc",
            "Meeting",
            1_745_000_000,
            1_745_003_600,
            Some("Quarterly planning"),
            Some("Conference Room A"),
            false,
        );
        assert!(ical.contains("DESCRIPTION:Quarterly planning"));
        assert!(ical.contains("LOCATION:Conference Room A"));
    }

    #[test]
    fn fold_line_wraps_at_75_bytes() {
        let long = "DESCRIPTION:".to_string() + &"x".repeat(80);
        let folded = fold_line(&long);
        // Every physical line after fold must be ≤ 75 octets
        for physical_line in folded.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(
                physical_line.len() <= 75,
                "line too long: {} bytes: {physical_line}",
                physical_line.len()
            );
        }
    }

    #[test]
    fn add_exdate_inserts_before_end_vevent() {
        let ical = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:rec-1\r\nRRULE:FREQ=WEEKLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let patched = add_exdate(ical, "20260422T090000Z");
        assert!(patched.contains("EXDATE:20260422T090000Z\r\nEND:VEVENT"));
    }

    #[test]
    fn add_recurrence_override_appends_vevent() {
        let parent = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:weekly-1\r\nRRULE:FREQ=WEEKLY\r\nDTSTART:20260415T090000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let patched = add_recurrence_override(
            parent,
            "20260422T090000Z",
            "Standup — renamed",
            1_745_600_000,
            1_745_603_600,
            None,
            None,
        );
        assert!(patched.contains("RECURRENCE-ID:20260422T090000Z"));
        assert!(patched.contains("SUMMARY:Standup — renamed"));
        // The original RRULE event must still be present
        assert!(patched.contains("RRULE:FREQ=WEEKLY"));
        // Two END:VEVENT (parent + override)
        assert_eq!(patched.matches("END:VEVENT").count(), 2);
    }
}
