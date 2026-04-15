use chrono::{TimeZone, Utc};

fn main() {
    let dt = Utc.with_ymd_and_hms(2026, 4, 22, 9, 30, 0).unwrap();
    let rfc = dt.to_rfc3339();
    println!("to_rfc3339() output: {}", rfc);
    println!("Matches +00:00? {}", rfc == "2026-04-22T09:30:00+00:00");
    println!("Matches Z? {}", rfc == "2026-04-22T09:30:00Z");
}
