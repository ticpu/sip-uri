use std::fmt;
use std::str::FromStr;

use crate::error::ParseUriError;
use crate::sip_uri::SipUri;
use crate::tel_uri::TelUri;
use crate::urn_uri::UrnUri;

/// A parsed URI: SIP/SIPS, tel, URN, or an opaque URI with an unrecognized scheme.
///
/// The `Other` variant stores the raw URI string for schemes this crate does
/// not parse (e.g. `http:`, `https:`, `data:`). This allows SIP header values
/// like `Call-Info` to round-trip without rejecting non-SIP URIs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Uri {
    /// SIP or SIPS URI.
    Sip(SipUri),
    /// tel: URI.
    Tel(TelUri),
    /// URN (Uniform Resource Name).
    Urn(UrnUri),
    /// URI with an unrecognized scheme, stored as-is.
    Other(String),
}

impl Uri {
    /// If this is a SIP/SIPS URI, return a reference to it.
    pub fn as_sip(&self) -> Option<&SipUri> {
        match self {
            Uri::Sip(u) => Some(u),
            _ => None,
        }
    }

    /// If this is a tel: URI, return a reference to it.
    pub fn as_tel(&self) -> Option<&TelUri> {
        match self {
            Uri::Tel(u) => Some(u),
            _ => None,
        }
    }

    /// If this is a URN, return a reference to it.
    pub fn as_urn(&self) -> Option<&UrnUri> {
        match self {
            Uri::Urn(u) => Some(u),
            _ => None,
        }
    }

    /// If this is an unrecognized scheme, return the raw URI string.
    pub fn as_other(&self) -> Option<&str> {
        match self {
            Uri::Other(s) => Some(s),
            _ => None,
        }
    }

    /// The scheme of this URI (lowercase).
    pub fn scheme(&self) -> &str {
        match self {
            Uri::Sip(u) => match u.scheme() {
                crate::Scheme::Sip => "sip",
                crate::Scheme::Sips => "sips",
            },
            Uri::Tel(_) => "tel",
            Uri::Urn(_) => "urn",
            Uri::Other(s) => s
                .find(':')
                .map(|i| &s[..i])
                .unwrap_or(s),
        }
    }
}

impl From<SipUri> for Uri {
    fn from(u: SipUri) -> Self {
        Uri::Sip(u)
    }
}

impl From<TelUri> for Uri {
    fn from(u: TelUri) -> Self {
        Uri::Tel(u)
    }
}

impl From<UrnUri> for Uri {
    fn from(u: UrnUri) -> Self {
        Uri::Urn(u)
    }
}

impl FromStr for Uri {
    type Err = ParseUriError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "*" {
            return Err(ParseUriError(
                "wildcard '*' is not a URI; handle it at the protocol layer (Contact: * or OPTIONS * SIP/2.0)".into(),
            ));
        }

        // Detect scheme by scanning to first `:`
        let colon = s
            .find(':')
            .ok_or_else(|| ParseUriError("missing scheme".into()))?;
        let scheme = &s[..colon];

        if scheme.eq_ignore_ascii_case("tel") {
            Ok(Uri::Tel(s.parse::<TelUri>()?))
        } else if scheme.eq_ignore_ascii_case("sip") || scheme.eq_ignore_ascii_case("sips") {
            Ok(Uri::Sip(s.parse::<SipUri>()?))
        } else if scheme.eq_ignore_ascii_case("urn") {
            Ok(Uri::Urn(s.parse::<UrnUri>()?))
        } else {
            Ok(Uri::Other(s.to_string()))
        }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Uri::Sip(u) => write!(f, "{u}"),
            Uri::Tel(u) => write!(f, "{u}"),
            Uri::Urn(u) => write!(f, "{u}"),
            Uri::Other(s) => write!(f, "{s}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_sip() {
        let uri: Uri = "sip:alice@example.com"
            .parse()
            .unwrap();
        assert!(uri
            .as_sip()
            .is_some());
        assert!(uri
            .as_tel()
            .is_none());
        assert!(uri
            .as_urn()
            .is_none());
    }

    #[test]
    fn dispatch_sips() {
        let uri: Uri = "sips:bob@secure.example.com"
            .parse()
            .unwrap();
        assert!(uri
            .as_sip()
            .is_some());
    }

    #[test]
    fn dispatch_tel() {
        let uri: Uri = "tel:+15551234567"
            .parse()
            .unwrap();
        assert!(uri
            .as_tel()
            .is_some());
        assert!(uri
            .as_sip()
            .is_none());
    }

    #[test]
    fn dispatch_urn() {
        let uri: Uri = "urn:service:sos"
            .parse()
            .unwrap();
        assert!(uri
            .as_urn()
            .is_some());
        assert!(uri
            .as_sip()
            .is_none());
        assert!(uri
            .as_tel()
            .is_none());
    }

    #[test]
    fn unknown_scheme_stored_as_other() {
        let uri: Uri = "http://example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.as_other(), Some("http://example.com"));
        assert_eq!(uri.scheme(), "http");
        assert!(uri
            .as_sip()
            .is_none());
        assert!(uri
            .as_tel()
            .is_none());
        assert!(uri
            .as_urn()
            .is_none());
    }

    #[test]
    fn other_display_roundtrip() {
        let input = "https://example.com/photo.jpg";
        let uri: Uri = input
            .parse()
            .unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn missing_scheme_fails() {
        assert!("no-colon-here"
            .parse::<Uri>()
            .is_err());
    }

    #[test]
    fn display_roundtrip() {
        let input = "sip:alice@example.com;transport=tcp";
        let uri: Uri = input
            .parse()
            .unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn display_roundtrip_urn() {
        let input = "urn:service:sos";
        let uri: Uri = input
            .parse()
            .unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn from_sip_uri() {
        let sip: SipUri = "sip:alice@example.com"
            .parse()
            .unwrap();
        let uri: Uri = sip.into();
        assert!(uri
            .as_sip()
            .is_some());
    }

    #[test]
    fn from_urn_uri() {
        let urn: UrnUri = "urn:service:sos"
            .parse()
            .unwrap();
        let uri: Uri = urn.into();
        assert!(uri
            .as_urn()
            .is_some());
    }
}
