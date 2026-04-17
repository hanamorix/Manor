//! 24h cache of GoCardless /institutions responses, keyed by country.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

const TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachedInstitution {
    pub country: String,
    pub institution_id: String,
    pub name: String,
    pub bic: Option<String>,
    pub logo_url: Option<String>,
    pub max_historical_days: i64,
    pub access_valid_for_days: i64,
}

impl CachedInstitution {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            country: row.get("country")?,
            institution_id: row.get("institution_id")?,
            name: row.get("name")?,
            bic: row.get("bic")?,
            logo_url: row.get("logo_url")?,
            max_historical_days: row.get("max_historical_days")?,
            access_valid_for_days: row.get("access_valid_for_days")?,
        })
    }
}

/// Returns cached rows for a country if any were fetched within the last 24h.
/// Empty vec means "cache miss or stale — caller should re-fetch".
pub fn get_fresh(conn: &Connection, country: &str) -> Result<Vec<CachedInstitution>> {
    let cutoff = Utc::now().timestamp() - TTL_SECONDS;
    let mut stmt = conn.prepare(
        "SELECT country, institution_id, name, bic, logo_url,
                max_historical_days, access_valid_for_days
         FROM gocardless_institution_cache
         WHERE country = ?1 AND fetched_at >= ?2
         ORDER BY name ASC",
    )?;
    let rows = stmt
        .query_map(params![country, cutoff], CachedInstitution::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Replaces the cached rows for a country in one transaction.
pub fn replace_for_country(
    conn: &mut Connection,
    country: &str,
    rows: &[CachedInstitution],
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM gocardless_institution_cache WHERE country = ?1",
        params![country],
    )?;
    let now = Utc::now().timestamp();
    for r in rows {
        tx.execute(
            "INSERT INTO gocardless_institution_cache
                 (country, institution_id, name, bic, logo_url,
                  max_historical_days, access_valid_for_days, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                country, r.institution_id, r.name, r.bic, r.logo_url,
                r.max_historical_days, r.access_valid_for_days, now,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::db;
    use tempfile::tempdir;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn fresh_cache_round_trip() {
        let (_d, mut conn) = test_conn();
        let rows = vec![sample("GB", "BARCLAYS", "Barclays")];
        replace_for_country(&mut conn, "GB", &rows).unwrap();

        let listed = get_fresh(&conn, "GB").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Barclays");
    }

    #[test]
    fn stale_cache_returns_empty() {
        let (_d, mut conn) = test_conn();
        let rows = vec![sample("GB", "BARCLAYS", "Barclays")];
        replace_for_country(&mut conn, "GB", &rows).unwrap();

        let stale_cutoff = Utc::now().timestamp() - (TTL_SECONDS + 60);
        conn.execute(
            "UPDATE gocardless_institution_cache SET fetched_at = ?1",
            params![stale_cutoff],
        ).unwrap();

        let listed = get_fresh(&conn, "GB").unwrap();
        assert!(listed.is_empty());
    }

    #[test]
    fn replace_is_per_country() {
        let (_d, mut conn) = test_conn();
        replace_for_country(&mut conn, "GB", &[sample("GB", "BARCLAYS", "Barclays")]).unwrap();
        replace_for_country(&mut conn, "FR", &[sample("FR", "BNP", "BNP Paribas")]).unwrap();
        replace_for_country(&mut conn, "GB", &[sample("GB", "MONZO", "Monzo")]).unwrap();

        let gb = get_fresh(&conn, "GB").unwrap();
        let fr = get_fresh(&conn, "FR").unwrap();
        assert_eq!(gb.len(), 1);
        assert_eq!(gb[0].institution_id, "MONZO");
        assert_eq!(fr.len(), 1);
        assert_eq!(fr[0].institution_id, "BNP");
    }

    fn sample(country: &str, id: &str, name: &str) -> CachedInstitution {
        CachedInstitution {
            country: country.into(),
            institution_id: id.into(),
            name: name.into(),
            bic: None,
            logo_url: None,
            max_historical_days: 180,
            access_valid_for_days: 180,
        }
    }
}
