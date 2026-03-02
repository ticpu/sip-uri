use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

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
        let (host, consumed) = Host::parse_from_uri("172.21.55.55:5060").unwrap();
        assert_eq!(host, Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)));
        assert_eq!(consumed, 12);
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
    fn empty_host_fails() {
        assert!(Host::parse_from_uri("").is_err());
        assert!(Host::parse_from_uri(":5060").is_err());
    }
}
