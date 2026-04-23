//! CSV import with named bank presets.
//!
//! Each preset maps its bank's CSV schema to the canonical Manor row
//! (date, amount, description). Amounts end up in pence, signed (negative = debit).

use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BankPreset {
    Monzo,
    Starling,
    Barclays,
    Hsbc,
    Natwest,
    Generic,
}

impl BankPreset {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "monzo" => Some(Self::Monzo),
            "starling" => Some(Self::Starling),
            "barclays" => Some(Self::Barclays),
            "hsbc" => Some(Self::Hsbc),
            "natwest" => Some(Self::Natwest),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewRow {
    pub date: i64,
    pub amount_pence: i64,
    pub description: String,
    pub suggested_category_id: Option<i64>,
    pub duplicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub inserted: usize,
    pub skipped_duplicates: usize,
    pub skipped_errors: usize,
}

/// Generic CSV parser dispatch. Returns rows with suggested categories + dup flags.
pub fn parse_preview(
    conn: &Connection,
    preset: BankPreset,
    csv_bytes: &[u8],
    generic_cols: Option<GenericCols>,
) -> Result<Vec<PreviewRow>> {
    let raw = match preset {
        BankPreset::Monzo => {
            parse_signed_amount(csv_bytes, "Date", "Amount", &["Name", "Description"])?
        }
        BankPreset::Starling => {
            parse_signed_amount(csv_bytes, "Date", "Amount (GBP)", &["Counter Party"])?
        }
        BankPreset::Barclays => parse_signed_amount(csv_bytes, "Date", "Amount", &["Memo"])?,
        BankPreset::Hsbc => parse_debit_credit(
            csv_bytes,
            "Date",
            "Debit Amount",
            "Credit Amount",
            &["Transaction Description"],
        )?,
        BankPreset::Natwest => parse_signed_amount(
            csv_bytes,
            "Date",
            "Value",
            &["Transaction type", "Description"],
        )?,
        BankPreset::Generic => {
            let c =
                generic_cols.ok_or_else(|| anyhow!("generic preset requires column indices"))?;
            parse_generic(csv_bytes, c)?
        }
    };

    let categories = manor_core::ledger::category::list(conn).unwrap_or_default();
    let out = raw
        .into_iter()
        .map(|r| {
            let suggested = categorize(&r.description, &categories);
            let duplicate =
                is_duplicate(conn, r.date, r.amount_pence, &r.description).unwrap_or(false);
            PreviewRow {
                date: r.date,
                amount_pence: r.amount_pence,
                description: r.description,
                suggested_category_id: suggested,
                duplicate,
            }
        })
        .collect();
    Ok(out)
}

/// Insert all non-duplicate rows. Uses a single sqlite transaction.
pub fn do_import(conn: &mut Connection, rows: Vec<PreviewRow>) -> Result<ImportResult> {
    let mut inserted = 0usize;
    let mut skipped_duplicates = 0usize;
    let mut skipped_errors = 0usize;
    let now = Utc::now().timestamp();
    let tx = conn.transaction()?;
    for row in rows {
        if row.duplicate {
            skipped_duplicates += 1;
            continue;
        }
        let res = tx.execute(
            "INSERT INTO ledger_transaction
             (amount_pence, currency, description, merchant,
              category_id, date, source, note, created_at)
             VALUES (?1, 'GBP', ?2, NULL, ?3, ?4, 'csv_import', NULL, ?5)",
            params![
                row.amount_pence,
                row.description,
                row.suggested_category_id,
                row.date,
                now
            ],
        );
        match res {
            Ok(_) => inserted += 1,
            Err(_) => skipped_errors += 1,
        }
    }
    tx.commit()?;
    Ok(ImportResult {
        inserted,
        skipped_duplicates,
        skipped_errors,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GenericCols {
    pub date: usize,
    pub amount: usize,
    pub description: usize,
}

#[derive(Debug)]
struct RawRow {
    date: i64,
    amount_pence: i64,
    description: String,
}

fn parse_signed_amount(
    csv_bytes: &[u8],
    date_col: &str,
    amount_col: &str,
    desc_cols: &[&str],
) -> Result<Vec<RawRow>> {
    let mut rdr = csv::Reader::from_reader(csv_bytes);
    let headers = rdr.headers()?.clone();
    let date_idx = header_index(&headers, date_col)?;
    let amount_idx = header_index(&headers, amount_col)?;
    let desc_idxs: Vec<usize> = desc_cols
        .iter()
        .filter_map(|c| header_index(&headers, c).ok())
        .collect();
    if desc_idxs.is_empty() {
        return Err(anyhow!("no description columns found ({:?})", desc_cols));
    }

    let mut out = Vec::new();
    for rec in rdr.records() {
        let Ok(rec) = rec else { continue };
        let Ok(date) = parse_date(rec.get(date_idx).unwrap_or("")) else {
            continue;
        };
        let Ok(amt) = parse_amount_pence(rec.get(amount_idx).unwrap_or("")) else {
            continue;
        };
        let description = desc_idxs
            .iter()
            .filter_map(|i| rec.get(*i))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        out.push(RawRow {
            date,
            amount_pence: amt,
            description,
        });
    }
    Ok(out)
}

fn parse_debit_credit(
    csv_bytes: &[u8],
    date_col: &str,
    debit_col: &str,
    credit_col: &str,
    desc_cols: &[&str],
) -> Result<Vec<RawRow>> {
    let mut rdr = csv::Reader::from_reader(csv_bytes);
    let headers = rdr.headers()?.clone();
    let date_idx = header_index(&headers, date_col)?;
    let debit_idx = header_index(&headers, debit_col)?;
    let credit_idx = header_index(&headers, credit_col)?;
    let desc_idxs: Vec<usize> = desc_cols
        .iter()
        .filter_map(|c| header_index(&headers, c).ok())
        .collect();

    let mut out = Vec::new();
    for rec in rdr.records() {
        let Ok(rec) = rec else { continue };
        let Ok(date) = parse_date(rec.get(date_idx).unwrap_or("")) else {
            continue;
        };
        let debit = parse_amount_pence(rec.get(debit_idx).unwrap_or("")).unwrap_or(0);
        let credit = parse_amount_pence(rec.get(credit_idx).unwrap_or("")).unwrap_or(0);
        let amount = if debit != 0 {
            -debit.abs()
        } else if credit != 0 {
            credit.abs()
        } else {
            continue;
        };
        let description = desc_idxs
            .iter()
            .filter_map(|i| rec.get(*i))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        out.push(RawRow {
            date,
            amount_pence: amount,
            description,
        });
    }
    Ok(out)
}

fn parse_generic(csv_bytes: &[u8], cols: GenericCols) -> Result<Vec<RawRow>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(csv_bytes);
    let mut out = Vec::new();
    for rec in rdr.records().skip(1) {
        let Ok(rec) = rec else { continue };
        let Ok(date) = parse_date(rec.get(cols.date).unwrap_or("")) else {
            continue;
        };
        let Ok(amt) = parse_amount_pence(rec.get(cols.amount).unwrap_or("")) else {
            continue;
        };
        let description = rec.get(cols.description).unwrap_or("").to_string();
        out.push(RawRow {
            date,
            amount_pence: amt,
            description,
        });
    }
    Ok(out)
}

fn header_index(headers: &csv::StringRecord, col: &str) -> Result<usize> {
    headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(col))
        .ok_or_else(|| anyhow!("missing header '{col}'"))
}

fn parse_date(s: &str) -> Result<i64> {
    let s = s.trim();
    // Try ISO (YYYY-MM-DD), then UK (DD/MM/YYYY), then slash-ISO (YYYY/MM/DD).
    for fmt in &["%Y-%m-%d", "%d/%m/%Y", "%Y/%m/%d", "%d-%m-%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(Utc
                .with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0)
                .single()
                .context("ymd out of range")?
                .timestamp());
        }
    }
    Err(anyhow!("unknown date format: {s}"))
}

fn parse_amount_pence(s: &str) -> Result<i64> {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(*c, ' ' | '£' | ',' | '\''))
        .collect();
    if cleaned.is_empty() {
        return Err(anyhow!("empty amount"));
    }
    let f: f64 = cleaned
        .parse()
        .map_err(|e| anyhow!("bad amount '{s}': {e}"))?;
    Ok((f * 100.0).round() as i64)
}

fn is_duplicate(
    conn: &Connection,
    date: i64,
    amount_pence: i64,
    description: &str,
) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ledger_transaction
         WHERE date = ?1 AND amount_pence = ?2 AND LOWER(description) = LOWER(?3)
           AND deleted_at IS NULL",
        params![date, amount_pence, description],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn categorize(
    description: &str,
    categories: &[manor_core::ledger::category::Category],
) -> Option<i64> {
    let up = description.to_uppercase();
    let find = |name: &str| {
        categories
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .map(|c| c.id)
    };
    const GROCERIES: &[&str] = &[
        "TESCO",
        "SAINSBURY",
        "WAITROSE",
        "ALDI",
        "LIDL",
        "ASDA",
        "MORRISONS",
    ];
    const EATING: &[&str] = &[
        "UBER EATS",
        "DELIVEROO",
        "JUST EAT",
        "MCDONALD",
        "KFC",
        "NANDO",
    ];
    const TRANSPORT: &[&str] = &["TFL", "UBER", "NATIONAL RAIL", "TRAINLINE"];
    const SUBS: &[&str] = &[
        "NETFLIX",
        "SPOTIFY",
        "AMAZON PRIME",
        "DISNEY",
        "APPLE",
        "O2",
        "EE",
        "VODAFONE",
        "THREE",
        "SKY",
        "BT",
        "VIRGIN",
    ];
    const HEALTH: &[&str] = &["BOOTS", "PHARMACY", "NHS", "DENTIST"];
    const INCOME: &[&str] = &["PAYROLL", "SALARY", "WAGES"];

    if GROCERIES.iter().any(|k| up.contains(k)) {
        return find("Groceries");
    }
    if EATING.iter().any(|k| up.contains(k)) {
        return find("Eating Out");
    }
    if TRANSPORT.iter().any(|k| up.contains(k)) {
        return find("Transport");
    }
    if SUBS.iter().any(|k| up.contains(k)) {
        return find("Subscriptions");
    }
    if HEALTH.iter().any(|k| up.contains(k)) {
        return find("Health");
    }
    if INCOME.iter().any(|k| up.contains(k)) {
        return find("Income");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn parses_monzo_signed_amount() {
        let csv = b"Date,Amount,Name,Description\n2026-04-10,-12.50,Tesco,Express\n2026-04-11,1500.00,Payroll Acme,\n";
        let (_d, conn) = fresh_conn();
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount_pence, -1250);
        assert!(rows[0].description.contains("Tesco"));
        assert_eq!(rows[1].amount_pence, 150000);
    }

    #[test]
    fn parses_hsbc_split_debit_credit() {
        let csv = b"Date,Debit Amount,Credit Amount,Transaction Description\n10/04/2026,12.50,,TESCO\n11/04/2026,,1500.00,SALARY\n";
        let (_d, conn) = fresh_conn();
        let rows = parse_preview(&conn, BankPreset::Hsbc, csv, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount_pence, -1250);
        assert_eq!(rows[1].amount_pence, 150000);
    }

    #[test]
    fn suggests_category_by_keyword() {
        let csv = b"Date,Amount,Name,Description\n2026-04-10,-12.50,TESCO EXPRESS,Groceries\n";
        let (_d, conn) = fresh_conn();
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        assert_eq!(rows[0].suggested_category_id, Some(1)); // Groceries (id=1 from seeds)
    }

    #[test]
    fn flags_duplicates() {
        let (_d, mut conn) = fresh_conn();
        let now = Utc
            .with_ymd_and_hms(2026, 4, 10, 0, 0, 0)
            .unwrap()
            .timestamp();
        manor_core::ledger::transaction::insert(
            &conn,
            -1250,
            "GBP",
            "Tesco Express",
            None,
            None,
            now,
            None,
        )
        .unwrap();
        let csv = b"Date,Amount,Name,Description\n2026-04-10,-12.50,Tesco,Express\n";
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        assert!(rows[0].duplicate);

        let result = do_import(&mut conn, rows).unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(result.skipped_duplicates, 1);
    }

    #[test]
    fn do_import_inserts_non_duplicates() {
        let (_d, mut conn) = fresh_conn();
        let csv =
            b"Date,Amount,Name,Description\n2026-04-10,-12.50,Tesco,\n2026-04-11,-5.00,Uber,\n";
        let rows = parse_preview(&conn, BankPreset::Monzo, csv, None).unwrap();
        let r = do_import(&mut conn, rows).unwrap();
        assert_eq!(r.inserted, 2);
        let txns = manor_core::ledger::transaction::list_by_month(&conn, 2026, 4).unwrap();
        assert_eq!(txns.len(), 2);
        assert_eq!(txns[0].source, "csv_import");
    }
}
