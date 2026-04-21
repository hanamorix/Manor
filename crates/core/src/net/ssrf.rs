use std::net::IpAddr;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SsrfError {
    #[error("not a valid URL")]
    BadUrl,
    #[error("unsupported scheme (only http/https allowed)")]
    BadScheme,
    #[error("hostname resolves to a private or loopback address")]
    PrivateAddress,
    #[error("hostname could not be resolved")]
    Unresolvable,
}

/// Parse and vet a URL for safe outbound fetching from user-controlled input.
///
/// Rejects: non-http(s) schemes, IPs that resolve to loopback (127.0.0.0/8, ::1),
/// private networks (10/8, 172.16/12, 192.168/16, fc00::/7), link-local
/// (169.254/16, fe80::/10), multicast, unspecified (0.0.0.0, ::).
///
/// Resolves DNS synchronously via `ToSocketAddrs` — every resolved IP must pass.
/// Literal IP addresses are checked directly without DNS resolution.
pub fn vet_url(raw: &str) -> Result<Url, SsrfError> {
    let url = Url::parse(raw).map_err(|_| SsrfError::BadUrl)?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(SsrfError::BadScheme);
    }

    use url::Host;
    match url.host().ok_or(SsrfError::BadUrl)? {
        Host::Ipv4(v4) => {
            if is_blocked_ip(&IpAddr::V4(v4)) {
                return Err(SsrfError::PrivateAddress);
            }
        }
        Host::Ipv6(v6) => {
            if is_blocked_ip(&IpAddr::V6(v6)) {
                return Err(SsrfError::PrivateAddress);
            }
        }
        Host::Domain(host) => {
            let port = url.port_or_known_default().unwrap_or(80);
            use std::net::ToSocketAddrs;
            let addrs: Vec<_> = (host, port)
                .to_socket_addrs()
                .map_err(|_| SsrfError::Unresolvable)?
                .collect();
            if addrs.is_empty() {
                return Err(SsrfError::Unresolvable);
            }
            for a in &addrs {
                if is_blocked_ip(&a.ip()) {
                    return Err(SsrfError::PrivateAddress);
                }
            }
        }
    }
    Ok(url)
}

fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // 100.64/10 CGNAT, not covered by is_private
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 0x40)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                // is_unique_local / is_unicast_link_local are unstable — hand-check
                || (v6.segments()[0] & 0xfe00) == 0xfc00  // ULA fc00::/7
                || (v6.segments()[0] & 0xffc0) == 0xfe80  // link-local fe80::/10
                // Any IPv4-mapped address must re-check as v4
                || v6.to_ipv4_mapped().map(|v4| is_blocked_ip(&IpAddr::V4(v4))).unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        assert_eq!(vet_url("file:///etc/passwd").unwrap_err(), SsrfError::BadScheme);
        assert_eq!(vet_url("ssh://host").unwrap_err(), SsrfError::BadScheme);
        assert_eq!(vet_url("javascript:alert(1)").unwrap_err(), SsrfError::BadScheme);
    }

    #[test]
    fn rejects_loopback_literal() {
        assert_eq!(vet_url("http://127.0.0.1:11434/").unwrap_err(), SsrfError::PrivateAddress);
        assert_eq!(vet_url("http://[::1]/").unwrap_err(), SsrfError::PrivateAddress);
    }

    #[test]
    fn rejects_private_ranges() {
        assert_eq!(vet_url("http://10.0.0.1/").unwrap_err(), SsrfError::PrivateAddress);
        assert_eq!(vet_url("http://192.168.1.1/").unwrap_err(), SsrfError::PrivateAddress);
        assert_eq!(vet_url("http://172.16.0.1/").unwrap_err(), SsrfError::PrivateAddress);
    }

    #[test]
    fn rejects_link_local() {
        assert_eq!(vet_url("http://169.254.169.254/").unwrap_err(), SsrfError::PrivateAddress);
    }

    #[test]
    fn rejects_ipv4_mapped_loopback() {
        // ::ffff:127.0.0.1 — common SSRF bypass: v6 syntax pointing at v4 loopback.
        assert_eq!(
            vet_url("http://[::ffff:127.0.0.1]/").unwrap_err(),
            SsrfError::PrivateAddress,
        );
    }

    #[test]
    fn rejects_ipv4_mapped_private() {
        assert_eq!(
            vet_url("http://[::ffff:10.0.0.1]/").unwrap_err(),
            SsrfError::PrivateAddress,
        );
    }

    #[test]
    fn rejects_bad_url() {
        assert_eq!(vet_url("not a url").unwrap_err(), SsrfError::BadUrl);
        assert_eq!(vet_url("http://").unwrap_err(), SsrfError::BadUrl);
    }

    #[test]
    fn accepts_public_host() {
        // 1.1.1.1 is public, always resolvable as a literal. Uses real DNS on the
        // machine — if this runs offline it will fail; that's acceptable for this
        // test suite (local-first app).
        vet_url("https://1.1.1.1/").expect("1.1.1.1 must pass");
    }
}
