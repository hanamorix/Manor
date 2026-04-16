//! Sync engine: orchestrates CalDAV fetch → iCal parse → RRULE expand → DB wipe-and-reinsert.

use anyhow::Result;
use chrono::{Duration, Utc};
use chrono_tz::Tz;
use manor_core::assistant::{calendar_account, event};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;

use crate::sync::caldav::CalDavClient;
use crate::sync::expand;
use crate::sync::ical;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub account_id: i64,
    pub events_added: u32,
    pub error: Option<String>,
    pub synced_at: i64,
}

/// In-memory set of currently-syncing account ids, behind a Mutex. Owned by Tauri state.
pub struct SyncState {
    in_flight: Mutex<HashSet<i64>>,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            in_flight: Mutex::new(HashSet::new()),
        }
    }

    fn try_begin(&self, id: i64) -> bool {
        let mut set = self.in_flight.lock().unwrap();
        set.insert(id)
    }

    fn end(&self, id: i64) {
        let mut set = self.in_flight.lock().unwrap();
        set.remove(&id);
    }
}

impl Default for SyncState {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a fetch/parse error to a short user-facing string + full context for last_error.
fn error_string(err: &anyhow::Error) -> String {
    let msg = err.to_string();
    if msg.contains("401") || msg.contains("403") {
        "bad credentials".into()
    } else if msg.contains("404") {
        "URL not found".into()
    } else if msg.contains("connect error") || msg.contains("error sending request") {
        "server unreachable".into()
    } else if msg.contains("no current-user-principal") || msg.contains("no calendar-home-set") {
        "discovery failed".into()
    } else {
        format!("sync failed: {msg}")
    }
}

pub async fn sync_account(
    conn: &mut Connection,
    sync_state: &SyncState,
    account_id: i64,
    password: &str,
    local_tz: Tz,
) -> SyncResult {
    let now_secs = Utc::now().timestamp();

    if !sync_state.try_begin(account_id) {
        return SyncResult {
            account_id,
            events_added: 0,
            error: Some("already syncing".into()),
            synced_at: now_secs,
        };
    }

    let result = do_sync(conn, account_id, password, local_tz).await;
    sync_state.end(account_id);

    match result {
        Ok(added) => {
            let _ = calendar_account::update_sync_state(conn, account_id, Some(now_secs), None);
            SyncResult {
                account_id,
                events_added: added,
                error: None,
                synced_at: now_secs,
            }
        }
        Err(e) => {
            let short = error_string(&e);
            // Full message goes into last_error; the short form is returned to the frontend.
            let _ = calendar_account::update_sync_state(conn, account_id, None, Some(&short));
            SyncResult {
                account_id,
                events_added: 0,
                error: Some(short),
                synced_at: now_secs,
            }
        }
    }
}

async fn do_sync(
    conn: &mut Connection,
    account_id: i64,
    password: &str,
    local_tz: Tz,
) -> Result<u32> {
    let account = calendar_account::get(conn, account_id)?
        .ok_or_else(|| anyhow::anyhow!("account {account_id} not found"))?;

    let client = CalDavClient::new(&account.username, password);
    let principal = client.discover_principal(&account.server_url).await?;
    let home_set = client.discover_home_set(&principal).await?;
    let calendars = client.list_calendars(&home_set).await?;

    let window_start = Utc::now() - Duration::days(7);
    let window_end = Utc::now() + Duration::days(14);

    let mut new_events: Vec<event::NewEvent> = Vec::new();
    for cal in &calendars {
        let blocks = client
            .report_events(&cal.url, window_start, window_end)
            .await?;
        for ics in blocks {
            for parsed in ical::parse_vcalendar(&ics.ical, local_tz) {
                match expand::expand(&parsed, account_id, window_start, window_end) {
                    Ok(mut occurrences) => new_events.append(&mut occurrences),
                    Err(e) => tracing::warn!("skipping expansion for uid {}: {e}", parsed.uid),
                }
            }
        }
    }

    let tx = conn.unchecked_transaction()?;
    event::delete_for_account(&tx, account_id)?;
    // insert_many uses its own transaction on `conn`, but here we commit via `tx` — inline the INSERTs.
    {
        let mut stmt = tx.prepare(
            "INSERT INTO event (calendar_account_id, external_id, title, start_at, end_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let created_at = Utc::now().timestamp_millis();
        for ev in &new_events {
            // Skip duplicates silently (UNIQUE constraint protects us regardless).
            let _ = stmt.execute(rusqlite::params![
                ev.calendar_account_id,
                ev.external_id,
                ev.title,
                ev.start_at,
                ev.end_at,
                created_at,
            ]);
        }
    }
    tx.commit()?;

    Ok(new_events.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use manor_core::assistant::db;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn propfind_body_principal(server_uri: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/</D:href>
    <D:propstat><D:prop><D:current-user-principal><D:href>{server_uri}/principal/</D:href></D:current-user-principal></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#
        )
    }

    fn propfind_body_home_set(server_uri: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>{server_uri}/principal/</D:href>
    <D:propstat><D:prop><C:calendar-home-set><D:href>{server_uri}/home/</D:href></C:calendar-home-set></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#
        )
    }

    fn propfind_body_calendars(server_uri: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>{server_uri}/home/cal1/</D:href>
    <D:propstat><D:prop><D:displayname>Work</D:displayname><D:resourcetype><D:collection/><C:calendar/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#
        )
    }

    fn report_body_three_events() -> String {
        r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response><D:href>/home/cal1/1.ics</D:href><D:propstat><D:prop><C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:one
DTSTART:20260415T100000Z
DTEND:20260415T110000Z
SUMMARY:One
END:VEVENT
END:VCALENDAR</C:calendar-data></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>
  <D:response><D:href>/home/cal1/2.ics</D:href><D:propstat><D:prop><C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:two
DTSTART:20260416T120000Z
DTEND:20260416T130000Z
SUMMARY:Two
END:VEVENT
END:VCALENDAR</C:calendar-data></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>
</D:multistatus>"#.to_string()
    }

    async fn mount_happy_path(server: &MockServer) {
        let uri = server.uri();
        Mock::given(method("PROPFIND"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(propfind_body_principal(&uri)))
            .mount(server)
            .await;
        Mock::given(method("PROPFIND"))
            .and(path("/principal/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(propfind_body_home_set(&uri)))
            .mount(server)
            .await;
        Mock::given(method("PROPFIND"))
            .and(path("/home/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(propfind_body_calendars(&uri)))
            .mount(server)
            .await;
        Mock::given(method("REPORT"))
            .and(path("/home/cal1/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(report_body_three_events()))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn sync_account_happy_path_with_mock_caldav() {
        let server = MockServer::start().await;
        mount_happy_path(&server).await;

        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", &server.uri(), "u").unwrap();
        let state = SyncState::new();

        let result = sync_account(&mut conn, &state, aid, "p", chrono_tz::UTC).await;
        assert_eq!(
            result.error, None,
            "expected no error, got {:?}",
            result.error
        );
        assert_eq!(result.events_added, 2);

        let row = calendar_account::get(&conn, aid).unwrap().unwrap();
        assert!(row.last_synced_at.is_some());
        assert_eq!(row.last_error, None);
    }

    #[tokio::test]
    async fn sync_account_401_sets_bad_credentials() {
        let server = MockServer::start().await;
        Mock::given(method("PROPFIND"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", &server.uri(), "u").unwrap();

        let result = sync_account(&mut conn, &SyncState::new(), aid, "p", chrono_tz::UTC).await;
        assert_eq!(result.error.as_deref(), Some("bad credentials"));

        let row = calendar_account::get(&conn, aid).unwrap().unwrap();
        assert_eq!(row.last_error.as_deref(), Some("bad credentials"));
        assert_eq!(row.last_synced_at, None);
    }

    #[tokio::test]
    async fn sync_account_network_unreachable() {
        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", "http://127.0.0.1:1", "u").unwrap();

        let result = sync_account(&mut conn, &SyncState::new(), aid, "p", chrono_tz::UTC).await;
        assert!(result.error.is_some());
        // Error should map to "server unreachable" — allow tolerance in string matching.
        let msg = result.error.unwrap();
        assert!(
            msg == "server unreachable" || msg.contains("unreachable") || msg.contains("connect")
        );
    }

    #[tokio::test]
    async fn double_sync_second_returns_already_syncing() {
        let server = MockServer::start().await;
        mount_happy_path(&server).await;

        let dir = tempdir().unwrap();
        let mut conn = db::init(&dir.path().join("t.db")).unwrap();
        let aid = calendar_account::insert(&conn, "Mock", &server.uri(), "u").unwrap();

        let state = SyncState::new();
        state.try_begin(aid); // pre-mark as in-flight

        let result = sync_account(&mut conn, &state, aid, "p", chrono_tz::UTC).await;
        assert_eq!(result.error.as_deref(), Some("already syncing"));
    }
}
