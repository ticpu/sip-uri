use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::canon;

/// Host component of a SIP URI.
///
/// IPv6 addresses are stored without brackets; [`fmt::Display`] adds them,
/// and [`Host::bare`] renders without. A URI holds a hostname that reads as
/// an IPv4 address as [`Host::IPv4`], as [`Host::from_hostname`] does.
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

/// DNS hostname in canonical form, so equal names compare and hash equal
/// however they were written.
///
/// Construction lowercases, decodes escaped unreserved characters and escapes
/// every other byte outside them; it does not otherwise validate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hostname(String);

impl Hostname {
    /// The hostname, lowercase.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Hostname {
    fn from(s: String) -> Self {
        Hostname::from(s.as_str())
    }
}

impl From<&str> for Hostname {
    fn from(s: &str) -> Self {
        Hostname(canon::canonize_hostname(s))
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
    /// [`Host::IPv4`] when the canonical name is a dotted-quad address as
    /// [`Ipv4Addr`]'s `FromStr` reads it, leading zeros refused; otherwise
    /// [`Host::Hostname`].
    ///
    /// URI constructors hold every host this way, so a URI built from
    /// `Host::Hostname("198.51.100.1".into())` equals the parsed one.
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use sip_uri_types::Host;
    ///
    /// assert_eq!(
    ///     Host::from_hostname("198.51.100.1"),
    ///     Host::IPv4(Ipv4Addr::new(198, 51, 100, 1))
    /// );
    /// assert!(matches!(Host::from_hostname("198.051.100.1"), Host::Hostname(_)));
    /// ```
    pub fn from_hostname(name: impl Into<Hostname>) -> Host {
        let name = name.into();
        let addr = name
            .as_str()
            .parse::<Ipv4Addr>();
        addr.map_or(Host::Hostname(name), Host::IPv4)
    }

    /// The host as URI constructors hold it: see [`Host::from_hostname`].
    pub(crate) fn normalized(self) -> Host {
        match self {
            Host::Hostname(name) => Host::from_hostname(name),
            host => host,
        }
    }

    /// Render without IPv6 brackets, for contexts that supply their own or take
    /// no brackets at all (SDP connection lines, URI parameter values, log text).
    ///
    /// ```
    /// use sip_uri_types::Host;
    ///
    /// let host = Host::IPv6("2001:db8::1".parse().unwrap());
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

    #[test]
    fn display_ipv6_has_brackets() {
        let host = Host::IPv6(Ipv6Addr::LOCALHOST);
        assert_eq!(host.to_string(), "[::1]");
    }
}
