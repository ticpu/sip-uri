use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::error::ParseHostError;
use crate::parse;

/// Host component of a SIP URI.
///
/// IPv6 addresses are stored without brackets; [`fmt::Display`] adds brackets
/// when formatting in URI context via [`Host::fmt_uri`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Host {
    /// IPv4 address.
    IPv4(Ipv4Addr),
    /// IPv6 address (stored without brackets).
    IPv6(Ipv6Addr),
    /// DNS hostname.
    Hostname(String),
}

impl Host {
    /// Parse a host from a URI string fragment.
    ///
    /// Handles `[IPv6]`, dotted-decimal IPv4, and DNS hostnames.
    /// Returns the parsed host and the number of bytes consumed.
    pub(crate) fn parse_from_uri(s: &str) -> Result<(Self, usize), String> {
        if s.is_empty() {
            return Err("empty host".into());
        }

        if s.starts_with('[') {
            // IPv6reference = "[" IPv6address "]"
            let end = s
                .find(']')
                .ok_or_else(|| "unclosed IPv6 bracket".to_string())?;
            let addr_str = &s[1..end];
            let addr: Ipv6Addr = addr_str
                .parse()
                .map_err(|e| format!("invalid IPv6 address: {e}"))?;
            Ok((Host::IPv6(addr), end + 1))
        } else {
            // Find end of host: terminated by `:`, `;`, `?`, `#`, `>`, or end of string
            // Must skip percent-encoded sequences when scanning
            let end = find_host_end(s);
            let host_str = &s[..end];

            if host_str.is_empty() {
                return Err("empty host".into());
            }

            // Percent-decode the host for parsing (unreserved chars decoded)
            let decoded = if host_str.contains('%') {
                parse::percent_decode(host_str, parse::is_unreserved)
            } else {
                host_str.to_string()
            };

            // Try IPv4 first — must be all digits and dots
            if decoded
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.')
            {
                if let Ok(addr) = decoded.parse::<Ipv4Addr>() {
                    return Ok((Host::IPv4(addr), end));
                }
            }

            // Validate hostname characters: alphanum, '-', '.'
            if !decoded
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
            {
                return Err(format!("invalid hostname character in '{decoded}'"));
            }

            Ok((Host::Hostname(decoded.to_ascii_lowercase()), end))
        }
    }
}

/// Find the end of a hostname in a URI string, handling percent-encoded sequences.
fn find_host_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b':' | b';' | b'?' | b'#' | b'>' => return i,
            b'%' if i + 2 < bytes.len() => {
                // Skip percent-encoded sequence
                i += 3;
            }
            _ => i += 1,
        }
    }
    i
}

impl Host {
    /// Format the host for use inside a URI (brackets around IPv6).
    pub fn fmt_uri(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::IPv4(addr) => write!(f, "{addr}"),
            Host::IPv6(addr) => write!(f, "[{addr}]"),
            Host::Hostname(name) => write!(f, "{name}"),
        }
    }

    /// Render without IPv6 brackets, for contexts that supply their own or take
    /// no brackets at all (SDP connection lines, URI parameter values, log text).
    ///
    /// ```
    /// use sip_uri::Host;
    ///
    /// let host: Host = "2001:db8::1".parse().unwrap();
    /// assert_eq!(host.to_string(), "[2001:db8::1]");
    /// assert_eq!(host.bare().to_string(), "2001:db8::1");
    /// ```
    pub fn bare(&self) -> Bare<'_> {
        Bare(self)
    }
}

/// [`fmt::Display`] wrapper that renders IPv6 without brackets.
///
/// Returned by [`Host::bare`].
#[derive(Debug, Clone, Copy)]
pub struct Bare<'a>(&'a Host);

impl fmt::Display for Bare<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Host::IPv4(addr) => write!(f, "{addr}"),
            Host::IPv6(addr) => write!(f, "{addr}"),
            Host::Hostname(name) => write!(f, "{name}"),
        }
    }
}

/// Parses a complete host, with no surrounding URI.
///
/// Accepts a hostname, a bare IPv4 or IPv6 address, and a bracketed IPv6
/// reference. Bracketed IPv4 (`[192.0.2.1]`) is rejected: brackets are the
/// `IPv6reference` production and the in-URI parser reads them the same way.
///
/// The whole string must be a host. Anything a URI would put after it — a port,
/// parameters, headers — is an error here, unlike parsing in URI position where
/// those terminate the host.
///
/// ```
/// use sip_uri::Host;
///
/// assert!("example.test".parse::<Host>().is_ok());
/// assert!("192.0.2.1".parse::<Host>().is_ok());
/// assert!("2001:db8::1".parse::<Host>().is_ok());
/// assert!("[2001:db8::1]".parse::<Host>().is_ok());
/// assert!("example.test:5060".parse::<Host>().is_err());
/// ```
impl FromStr for Host {
    type Err = ParseHostError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // A bare IPv6 has to be recognized up front: the URI parser stops the
        // host at the first `:`, which is inside the address here.
        if !s.starts_with('[') {
            if let Ok(addr) = s.parse::<Ipv6Addr>() {
                return Ok(Host::IPv6(addr));
            }
        }

        let (host, consumed) = Host::parse_from_uri(s).map_err(ParseHostError)?;
        if consumed != s.len() {
            return Err(ParseHostError(format!(
                "trailing content after host: '{}'",
                &s[consumed..]
            )));
        }
        Ok(host)
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_uri(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ipv4() {
        let (host, consumed) = Host::parse_from_uri("198.51.100.55:5060").unwrap();
        assert_eq!(host, Host::IPv4(Ipv4Addr::new(198, 51, 100, 55)));
        assert_eq!(consumed, 13);
    }

    #[test]
    fn parse_ipv6() {
        let (host, consumed) = Host::parse_from_uri("[::1]:56001").unwrap();
        assert_eq!(host, Host::IPv6(Ipv6Addr::LOCALHOST));
        assert_eq!(consumed, 5);
    }

    #[test]
    fn parse_ipv6_full() {
        let (host, consumed) = Host::parse_from_uri("[2001:db8::1]:5061").unwrap();
        assert_eq!(
            host,
            Host::IPv6(
                "2001:db8::1"
                    .parse::<Ipv6Addr>()
                    .unwrap()
            )
        );
        assert_eq!(consumed, 13);
    }

    #[test]
    fn parse_hostname() {
        let (host, consumed) = Host::parse_from_uri("example.com;transport=tcp").unwrap();
        assert_eq!(host, Host::Hostname("example.com".into()));
        assert_eq!(consumed, 11);
    }

    #[test]
    fn hostname_lowercased() {
        let (host, _) = Host::parse_from_uri("MY.DOMAIN").unwrap();
        assert_eq!(host, Host::Hostname("my.domain".into()));
    }

    #[test]
    fn display_ipv6_has_brackets() {
        let host = Host::IPv6(Ipv6Addr::LOCALHOST);
        assert_eq!(host.to_string(), "[::1]");
    }

    #[test]
    fn from_str_accepts_all_forms() {
        assert_eq!(
            "example.test"
                .parse::<Host>()
                .unwrap(),
            Host::Hostname("example.test".into())
        );
        assert_eq!(
            "192.0.2.1"
                .parse::<Host>()
                .unwrap(),
            Host::IPv4(Ipv4Addr::new(192, 0, 2, 1))
        );
        let v6 = Host::IPv6(
            "2001:db8::1"
                .parse::<Ipv6Addr>()
                .unwrap(),
        );
        assert_eq!(
            "2001:db8::1"
                .parse::<Host>()
                .unwrap(),
            v6
        );
        assert_eq!(
            "[2001:db8::1]"
                .parse::<Host>()
                .unwrap(),
            v6
        );
    }

    #[test]
    fn from_str_round_trips_both_renderers() {
        for input in ["example.test", "192.0.2.1", "2001:db8::1", "[2001:db8::1]"] {
            let host: Host = input
                .parse()
                .unwrap();
            assert_eq!(
                host.to_string()
                    .parse::<Host>()
                    .unwrap(),
                host
            );
            assert_eq!(
                host.bare()
                    .to_string()
                    .parse::<Host>()
                    .unwrap(),
                host
            );
        }
        let host: Host = "[2001:db8::1]"
            .parse()
            .unwrap();
        assert_eq!(host.to_string(), "[2001:db8::1]");
        assert_eq!(
            host.bare()
                .to_string(),
            "2001:db8::1"
        );
    }

    #[test]
    fn from_str_rejects_non_hosts() {
        for input in [
            "not a host:",
            "example.test:5060",
            "example.test;transport=tcp",
            "[192.0.2.1]",
            "[2001:db8::1",
            "",
        ] {
            assert!(
                input
                    .parse::<Host>()
                    .is_err(),
                "expected rejection of {input:?}"
            );
        }
    }

    #[test]
    fn empty_host_fails() {
        assert!(Host::parse_from_uri("").is_err());
        assert!(Host::parse_from_uri(":5060").is_err());
    }
}
