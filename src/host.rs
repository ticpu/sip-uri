use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::error::ParseError;
use crate::parse;
use crate::warning::{Component, Parsed, WarningCode, Warnings};

/// Host component of a SIP URI.
///
/// IPv6 addresses are stored without brackets; [`fmt::Display`] adds them,
/// and [`Host::bare`] renders without.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Host {
    /// IPv4 address.
    IPv4(Ipv4Addr),
    /// IPv6 address (stored without brackets).
    IPv6(Ipv6Addr),
    /// DNS hostname.
    Hostname(Hostname),
}

/// DNS hostname, lowercase by construction so equal names compare and hash
/// equal however they were written.
///
/// Construction does not validate; [`Host::parse_with_warnings`] reports
/// characters outside the hostname grammar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hostname(String);

impl Hostname {
    /// The hostname, lowercase.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Hostname {
    fn from(mut s: String) -> Self {
        s.make_ascii_lowercase();
        Hostname(s)
    }
}

impl From<&str> for Hostname {
    fn from(s: &str) -> Self {
        Hostname(s.to_ascii_lowercase())
    }
}

impl std::ops::Deref for Hostname {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for Hostname {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Hostname {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for Hostname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Host {
    /// Parse a host from a URI string fragment.
    ///
    /// Handles `[IPv6]`, dotted-decimal IPv4, and DNS hostnames.
    /// Returns the host, `None` when absent or unreadable, and the bytes consumed.
    pub(crate) fn parse_from_uri(s: &str, warnings: &mut Warnings) -> (Option<Self>, usize) {
        if let Some(inner) = s.strip_prefix('[') {
            // IPv6reference = "[" IPv6address "]"
            let Some(end) = inner.find(']') else {
                warnings.push(Component::Host, WarningCode::InvalidIpv6, s, 0);
                return (
                    None,
                    s.find([';', '?', '#'])
                        .unwrap_or(s.len()),
                );
            };
            return match inner[..end].parse::<Ipv6Addr>() {
                Ok(addr) => (Some(Host::IPv6(addr)), end + 2),
                Err(_) => {
                    warnings.push(Component::Host, WarningCode::InvalidIpv6, s, 0);
                    (None, end + 2)
                }
            };
        }

        {
            let end = find_host_end(s);
            let host_str = &s[..end];

            if host_str.is_empty() {
                warnings.push(Component::Host, WarningCode::MissingHost, s, 0);
                return (None, 0);
            }

            let decoded = if host_str.contains('%') {
                warnings.push(Component::Host, WarningCode::EscapedHost, host_str, 0);
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
                    return (Some(Host::IPv4(addr)), end);
                }
            }

            match decoded
                .bytes()
                .position(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.')))
            {
                Some(pos) if decoded.len() == host_str.len() => {
                    warnings.push(Component::Host, WarningCode::InvalidChar, host_str, pos)
                }
                Some(_) => warnings.push(Component::Host, WarningCode::InvalidChar, host_str, 0),
                None => warn_hostname_labels(&decoded, host_str, warnings),
            }

            (Some(Host::Hostname(Hostname::from(decoded))), end)
        }
    }
}

/// RFC 3261 §25: `hostname = *( domainlabel "." ) toplabel [ "." ]`, a label
/// neither empty nor hyphen-bounded, the toplabel starting with ALPHA.
///
/// `raw` is the host as written; positions point at its start when it was
/// escaped, since `name` offsets then no longer map onto the input.
fn warn_hostname_labels(name: &str, raw: &str, warnings: &mut Warnings) {
    let labels = name
        .strip_suffix('.')
        .unwrap_or(name);
    let offset = |label: &str| {
        if name.len() == raw.len() {
            label.as_ptr() as usize - name.as_ptr() as usize
        } else {
            0
        }
    };
    let mut last = None;
    for label in labels.split('.') {
        if label.is_empty() || label.starts_with('-') || label.ends_with('-') {
            warnings.push(
                Component::Host,
                WarningCode::InvalidHostLabel,
                raw,
                offset(label),
            );
            return;
        }
        last = Some(label);
    }
    if let Some(top) = last {
        if !top.as_bytes()[0].is_ascii_alphabetic() {
            warnings.push(
                Component::Host,
                WarningCode::NumericToplabel,
                raw,
                offset(top),
            );
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
/// Anything a URI would put after the host (a port, parameters, headers) is
/// dropped with a `TrailingContent` warning. Input with no readable host is the
/// only error.
///
/// ```
/// use sip_uri::{Host, WarningCode};
///
/// assert!("example.test".parse::<Host>().is_ok());
/// assert!("192.0.2.1".parse::<Host>().is_ok());
/// assert!("2001:db8::1".parse::<Host>().is_ok());
/// assert!("[2001:db8::1]".parse::<Host>().is_ok());
/// let parsed = Host::parse_with_warnings("example.test:5060").unwrap();
/// assert_eq!(parsed.warnings[0].code, WarningCode::TrailingContent);
/// assert!("[2001:db8::1".parse::<Host>().is_err());
/// ```
impl FromStr for Host {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_with_warnings(s).map(|parsed| parsed.value)
    }
}

impl Host {
    /// Parse, rejecting any grammar breach as [`ParseError::NonConformant`].
    pub fn parse_strict(input: &str) -> Result<Self, ParseError> {
        Self::parse_with_warnings(input)?.into_strict()
    }

    /// Parse a complete host as [`FromStr`] does, reporting accepted grammar
    /// breaches beside the value.
    pub fn parse_with_warnings(s: &str) -> Result<Parsed<Self>, ParseError> {
        let mut warnings = Warnings::new(s);
        // A bare IPv6 has to be recognized up front: the URI parser stops the
        // host at the first `:`, which is inside the address here.
        if !s.starts_with('[') {
            if let Ok(addr) = s.parse::<Ipv6Addr>() {
                return Ok(warnings.finish(Host::IPv6(addr)));
            }
        }

        let (host, consumed) = Host::parse_from_uri(s, &mut warnings);
        let Some(host) = host else {
            return Err(ParseError::Empty);
        };
        if consumed != s.len() {
            warnings.push(Component::Host, WarningCode::TrailingContent, s, consumed);
        }
        Ok(warnings.finish(host))
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::IPv4(addr) => write!(f, "{addr}"),
            Host::IPv6(addr) => write!(f, "[{addr}]"),
            Host::Hostname(name) => write!(f, "{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_uri(s: &str) -> Option<(Host, usize)> {
        let (host, consumed) = Host::parse_from_uri(s, &mut Warnings::new(s));
        host.map(|h| (h, consumed))
    }

    #[test]
    fn parse_ipv4() {
        let (host, consumed) = from_uri("198.51.100.55:5060").unwrap();
        assert_eq!(host, Host::IPv4(Ipv4Addr::new(198, 51, 100, 55)));
        assert_eq!(consumed, 13);
    }

    #[test]
    fn parse_ipv6() {
        let (host, consumed) = from_uri("[::1]:56001").unwrap();
        assert_eq!(host, Host::IPv6(Ipv6Addr::LOCALHOST));
        assert_eq!(consumed, 5);
    }

    #[test]
    fn parse_ipv6_full() {
        let (host, consumed) = from_uri("[2001:db8::1]:5061").unwrap();
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
        let (host, consumed) = from_uri("example.com;transport=tcp").unwrap();
        assert_eq!(host, Host::Hostname("example.com".into()));
        assert_eq!(consumed, 11);
    }

    #[test]
    fn hostname_lowercased() {
        let (host, _) = from_uri("MY.DOMAIN").unwrap();
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
    fn from_str_rejects_only_unreadable_hosts() {
        for input in ["[192.0.2.1]", "[2001:db8::1", ":5060", ""] {
            assert!(
                input
                    .parse::<Host>()
                    .is_err(),
                "expected rejection of {input:?}"
            );
        }
        for input in [
            "not a host:",
            "example.test:5060",
            "example.test;transport=tcp",
        ] {
            assert!(
                Host::parse_with_warnings(input)
                    .unwrap()
                    .has_warnings(),
                "expected warnings for {input:?}"
            );
        }
    }

    #[test]
    fn empty_host_is_none() {
        assert!(from_uri("").is_none());
        assert!(from_uri(":5060").is_none());
    }
}
