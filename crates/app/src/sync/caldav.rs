//! CalDAV HTTP client — discovery (PROPFIND) + event fetch (REPORT).

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use chrono::{DateTime, Utc};
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Method;

const PROPFIND: &str = "PROPFIND";
const REPORT: &str = "REPORT";

pub struct CalDavClient {
    http: reqwest::Client,
    username: String,
    password: String,
}

#[derive(Debug, Clone)]
pub struct CalendarInfo {
    pub url: String,
    pub display_name: Option<String>,
}

impl CalDavClient {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            username: username.into(),
            password: password.into(),
        }
    }

    fn auth_header(&self) -> HeaderValue {
        let creds = format!("{}:{}", self.username, self.password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(creds.as_bytes());
        HeaderValue::from_str(&format!("Basic {encoded}")).expect("header should never be bad")
    }

    async fn request_xml(
        &self,
        method: &str,
        url: &str,
        depth: Option<&str>,
        body: &str,
    ) -> Result<String> {
        let method_enum = Method::from_bytes(method.as_bytes())?;
        let mut req = self.http.request(method_enum, url);
        req = req.header(AUTHORIZATION, self.auth_header());
        req = req.header(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );
        if let Some(d) = depth {
            req = req.header("Depth", d);
        }
        req = req.body(body.to_string());

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() && status.as_u16() != 207 {
            let body = resp.text().await.unwrap_or_default();
            bail!("CalDAV {method} {url} returned {status}: {body}");
        }
        Ok(resp.text().await?)
    }

    /// Returns the current-user-principal href.
    pub async fn discover_principal(&self, server_url: &str) -> Result<String> {
        let body = r#"<?xml version="1.0"?>
<D:propfind xmlns:D="DAV:">
  <D:prop><D:current-user-principal/></D:prop>
</D:propfind>"#;
        let xml = self
            .request_xml(PROPFIND, server_url, Some("0"), body)
            .await?;
        extract_first_href(&xml, "current-user-principal")
            .ok_or_else(|| anyhow!("no current-user-principal in PROPFIND response"))
            .map(|href| absolutize(server_url, &href))
    }

    /// Returns the calendar-home-set href.
    pub async fn discover_home_set(&self, principal_url: &str) -> Result<String> {
        let body = r#"<?xml version="1.0"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-home-set/></D:prop>
</D:propfind>"#;
        let xml = self
            .request_xml(PROPFIND, principal_url, Some("0"), body)
            .await?;
        extract_first_href(&xml, "calendar-home-set")
            .ok_or_else(|| anyhow!("no calendar-home-set in PROPFIND response"))
            .map(|href| absolutize(principal_url, &href))
    }

    /// Lists calendar collections under the home-set.
    pub async fn list_calendars(&self, home_set_url: &str) -> Result<Vec<CalendarInfo>> {
        let body = r#"<?xml version="1.0"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
  </D:prop>
</D:propfind>"#;
        let xml = self
            .request_xml(PROPFIND, home_set_url, Some("1"), body)
            .await?;
        Ok(extract_calendar_collections(&xml, home_set_url))
    }

    /// Fetches events from a calendar URL within `[start, end)`.
    pub async fn report_events(
        &self,
        calendar_url: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<String>> {
        let start_s = start.format("%Y%m%dT%H%M%SZ").to_string();
        let end_s = end.format("%Y%m%dT%H%M%SZ").to_string();
        let body = format!(
            r#"<?xml version="1.0"?>
<C:calendar-query xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:D="DAV:">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT">
        <C:time-range start="{start_s}" end="{end_s}"/>
      </C:comp-filter>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#
        );
        let xml = self
            .request_xml(REPORT, calendar_url, Some("1"), &body)
            .await?;
        Ok(extract_calendar_data_blocks(&xml))
    }
}

/// Parse a PROPFIND response and return the first `<D:href>` under the named prop element.
fn extract_first_href(xml: &str, prop_local_name: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut inside_prop = false;
    let mut inside_href = false;
    let mut href = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                let name = local_name(e.name().as_ref());
                if name == prop_local_name {
                    inside_prop = true;
                }
                if inside_prop && name == "href" {
                    inside_href = true;
                    href.clear();
                }
            }
            Ok(XmlEvent::Text(t)) => {
                if inside_href {
                    href.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(XmlEvent::End(e)) => {
                let name = local_name(e.name().as_ref());
                if name == "href" && inside_href {
                    return Some(href.trim().to_string());
                }
                if name == prop_local_name {
                    inside_prop = false;
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Find all `<D:response>` elements that look like calendar collections (resourcetype has
/// `<C:calendar/>`) and return their href + displayname.
fn extract_calendar_collections(xml: &str, base_url: &str) -> Vec<CalendarInfo> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut in_response = false;
    let mut in_href = false;
    let mut in_resourcetype = false;
    let mut in_displayname = false;
    let mut saw_calendar_type = false;
    let mut cur_href = String::new();
    let mut cur_display = String::new();

    let mut out = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "response" => {
                        in_response = true;
                        saw_calendar_type = false;
                        cur_href.clear();
                        cur_display.clear();
                    }
                    "href" if in_response => {
                        in_href = true;
                        cur_href.clear();
                    }
                    "resourcetype" => in_resourcetype = true,
                    "displayname" if in_response => {
                        in_displayname = true;
                        cur_display.clear();
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Empty(e)) => {
                let name = local_name(e.name().as_ref());
                if in_resourcetype && name == "calendar" {
                    saw_calendar_type = true;
                }
            }
            Ok(XmlEvent::Text(t)) => {
                let text = t.unescape().unwrap_or_default();
                if in_href {
                    cur_href.push_str(&text);
                }
                if in_displayname {
                    cur_display.push_str(&text);
                }
            }
            Ok(XmlEvent::End(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "href" => in_href = false,
                    "resourcetype" => in_resourcetype = false,
                    "displayname" => in_displayname = false,
                    "response" => {
                        if in_response && saw_calendar_type && !cur_href.is_empty() {
                            out.push(CalendarInfo {
                                url: absolutize(base_url, cur_href.trim()),
                                display_name: if cur_display.trim().is_empty() {
                                    None
                                } else {
                                    Some(cur_display.trim().to_string())
                                },
                            });
                        }
                        in_response = false;
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    out
}

/// Extract every `<C:calendar-data>` text block from a REPORT response.
fn extract_calendar_data_blocks(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut in_data = false;
    let mut cur = String::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                if local_name(e.name().as_ref()) == "calendar-data" {
                    in_data = true;
                    cur.clear();
                }
            }
            Ok(XmlEvent::Text(t)) => {
                if in_data {
                    cur.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(XmlEvent::CData(t)) => {
                if in_data {
                    let s = String::from_utf8_lossy(&t);
                    cur.push_str(&s);
                }
            }
            Ok(XmlEvent::End(e)) => {
                if local_name(e.name().as_ref()) == "calendar-data" {
                    in_data = false;
                    if !cur.trim().is_empty() {
                        out.push(cur.clone());
                    }
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn local_name(full: &[u8]) -> String {
    match full.iter().position(|&b| b == b':') {
        Some(i) => String::from_utf8_lossy(&full[i + 1..]).to_string(),
        None => String::from_utf8_lossy(full).to_string(),
    }
}

/// Resolve `maybe_relative` against `base`. If `maybe_relative` is already absolute
/// (starts with `http://` or `https://`), return it unchanged. If it starts with `/`,
/// take the scheme+host from `base`. Otherwise return as-is (unusual; best effort).
fn absolutize(base: &str, maybe_relative: &str) -> String {
    if maybe_relative.starts_with("http://") || maybe_relative.starts_with("https://") {
        return maybe_relative.to_string();
    }
    if maybe_relative.starts_with('/') {
        if let Ok(parsed) = reqwest::Url::parse(base) {
            return format!(
                "{}://{}{}",
                parsed.scheme(),
                parsed.host_str().unwrap_or(""),
                maybe_relative
            );
        }
    }
    maybe_relative.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn discover_principal_parses_href() {
        let server = MockServer::start().await;
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/</D:href>
    <D:propstat><D:prop><D:current-user-principal><D:href>/12345/principal/</D:href></D:current-user-principal></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
        Mock::given(method("PROPFIND"))
            .and(header("Depth", "0"))
            .respond_with(
                ResponseTemplate::new(207)
                    .set_body_string(body)
                    .append_header("Content-Type", "application/xml"),
            )
            .mount(&server)
            .await;

        let client = CalDavClient::new("u", "p");
        let principal = client.discover_principal(&server.uri()).await.unwrap();
        assert!(principal.ends_with("/12345/principal/"));
    }

    #[tokio::test]
    async fn list_calendars_filters_to_calendar_resourcetype() {
        let server = MockServer::start().await;
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/home/cal1/</D:href>
    <D:propstat><D:prop><D:displayname>Work</D:displayname><D:resourcetype><D:collection/><C:calendar/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/home/notcal/</D:href>
    <D:propstat><D:prop><D:displayname>Inbox</D:displayname><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
        Mock::given(method("PROPFIND"))
            .and(header("Depth", "1"))
            .respond_with(ResponseTemplate::new(207).set_body_string(body))
            .mount(&server)
            .await;

        let client = CalDavClient::new("u", "p");
        let cals = client
            .list_calendars(&format!("{}/home/", server.uri()))
            .await
            .unwrap();
        assert_eq!(cals.len(), 1);
        assert!(cals[0].url.ends_with("/home/cal1/"));
        assert_eq!(cals[0].display_name.as_deref(), Some("Work"));
    }

    #[tokio::test]
    async fn report_events_extracts_calendar_data_blocks() {
        let server = MockServer::start().await;
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/home/cal1/1.ics</D:href>
    <D:propstat><D:prop>
      <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:u1
DTSTART:20260415T093000Z
DTEND:20260415T103000Z
SUMMARY:Boiler
END:VEVENT
END:VCALENDAR</C:calendar-data>
    </D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
        Mock::given(method("REPORT"))
            .respond_with(ResponseTemplate::new(207).set_body_string(body))
            .mount(&server)
            .await;

        let client = CalDavClient::new("u", "p");
        let start = Utc::now() - chrono::Duration::days(7);
        let end = Utc::now() + chrono::Duration::days(14);
        let ics = client
            .report_events(&format!("{}/home/cal1/", server.uri()), start, end)
            .await
            .unwrap();
        assert_eq!(ics.len(), 1);
        assert!(ics[0].contains("UID:u1"));
    }

    #[tokio::test]
    async fn authorization_header_present() {
        let server = MockServer::start().await;
        Mock::given(method("PROPFIND"))
            .and(header("Authorization", "Basic dTpw")) // base64("u:p") = "dTpw"
            .respond_with(ResponseTemplate::new(207).set_body_string(
                r#"<?xml version="1.0"?><D:multistatus xmlns:D="DAV:"><D:response><D:href>/</D:href><D:propstat><D:prop><D:current-user-principal><D:href>/p/</D:href></D:current-user-principal></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>"#))
            .mount(&server).await;

        let client = CalDavClient::new("u", "p");
        client.discover_principal(&server.uri()).await.unwrap();
    }
}
