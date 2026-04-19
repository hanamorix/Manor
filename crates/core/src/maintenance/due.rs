//! Pure date-math for maintenance schedules — compute next_due_date and classify into bands.

use super::DueBand;
use anyhow::Result;
use chrono::{Months, NaiveDate};

/// Compute next_due_date given last_done (or None → use fallback_start).
/// fallback_start is the schedule creation date when last_done is absent.
pub fn compute_next_due(
    last_done_date: Option<&str>,
    interval_months: i32,
    fallback_start: &str,
) -> Result<String> {
    let anchor = last_done_date.unwrap_or(fallback_start);
    let parsed = NaiveDate::parse_from_str(anchor, "%Y-%m-%d")?;
    let next = parsed
        .checked_add_months(Months::new(interval_months as u32))
        .ok_or_else(|| anyhow::anyhow!("date overflow adding {} months", interval_months))?;
    Ok(next.format("%Y-%m-%d").to_string())
}

pub fn classify(next_due_date: &str, today: &str) -> Result<DueBand> {
    let due = NaiveDate::parse_from_str(next_due_date, "%Y-%m-%d")?;
    let today_date = NaiveDate::parse_from_str(today, "%Y-%m-%d")?;
    let days = (due - today_date).num_days();
    Ok(match days {
        n if n <= 0 => DueBand::Overdue,
        n if n <= 7 => DueBand::DueThisWeek,
        n if n <= 30 => DueBand::Upcoming,
        _ => DueBand::Far,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_uses_fallback_when_never_done() {
        let next = compute_next_due(None, 12, "2025-06-15").unwrap();
        assert_eq!(next, "2026-06-15");
    }

    #[test]
    fn compute_month_end_edge_case() {
        // Jan 31 + 1 month = Feb 28 (2025 is not a leap year).
        let next = compute_next_due(Some("2025-01-31"), 1, "2025-01-01").unwrap();
        assert_eq!(next, "2025-02-28");
    }

    #[test]
    fn compute_month_end_edge_leap_year() {
        // Jan 31 + 1 month = Feb 29 (2024 is a leap year).
        let next = compute_next_due(Some("2024-01-31"), 1, "2024-01-01").unwrap();
        assert_eq!(next, "2024-02-29");
    }

    #[test]
    fn compute_multi_month_basic() {
        let next = compute_next_due(Some("2025-03-15"), 3, "2025-01-01").unwrap();
        assert_eq!(next, "2025-06-15");
    }

    #[test]
    fn classify_overdue_past() {
        assert_eq!(classify("2025-06-14", "2025-06-15").unwrap(), DueBand::Overdue);
    }

    #[test]
    fn classify_due_today_is_overdue() {
        assert_eq!(classify("2025-06-15", "2025-06-15").unwrap(), DueBand::Overdue);
    }

    #[test]
    fn classify_due_this_week() {
        assert_eq!(classify("2025-06-18", "2025-06-15").unwrap(), DueBand::DueThisWeek);
    }

    #[test]
    fn classify_upcoming() {
        assert_eq!(classify("2025-06-30", "2025-06-15").unwrap(), DueBand::Upcoming);
    }

    #[test]
    fn classify_far() {
        assert_eq!(classify("2025-08-01", "2025-06-15").unwrap(), DueBand::Far);
    }
}
