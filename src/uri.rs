use std::fmt;
use std::str::FromStr;

use crate::error::ParseError;
use crate::parse::{self, SchemeSplit};
use crate::sip_uri::Scheme;
use crate::sip_uri::SipUri;
use crate::tel_uri::TelUri;
use crate::urn_uri::UrnUri;
use crate::warning::{Component, Parsed, WarningCode, Warnings};

/// A parsed URI: SIP/SIPS, tel, URN, or an opaque URI with an unrecognized scheme.
///
/// The `Other` variant keeps text this crate does not parse (e.g. `http:`,
/// `https:`, `data:`, or text without a scheme), so header values like
/// `Call-Info` round-trip without rejecting non-SIP URIs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Uri {
    /// SIP or SIPS URI.
    Sip(SipUri),
    /// tel: URI.
    Tel(TelUri),
    /// URN (Uniform Resource Name).
    Urn(UrnUri),
    /// URI with an unrecognized scheme, or none.
    Other(OtherUri),
}

/// Text kept as sent except for its scheme, which is lowercased.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OtherUri {
    raw: String,
    scheme_end: Option<usize>,
}

impl OtherUri {
    fn new(s: &str) -> Self {
        match parse::split_scheme(s) {
            SchemeSplit::Named(scheme, rest) => OtherUri {
                raw: format!("{}:{rest}", scheme.to_ascii_lowercase()),
                scheme_end: Some(scheme.len()),
            },
            SchemeSplit::Invalid | SchemeSplit::Absent => OtherUri {
                raw: s.to_string(),
                scheme_end: None,
            },
        }
    }

    /// The whole URI, with its scheme lowercased.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// The scheme, lowercase, or `None` when the text has none.
    pub fn scheme(&self) -> Option<&str> {
        self.scheme_end
            .map(|end| &self.raw[..end])
    }
}

impl fmt::Display for OtherUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
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

    /// If this is an unrecognized scheme, return the URI text.
    pub fn as_other(&self) -> Option<&str> {
        match self {
            Uri::Other(o) => Some(o.as_str()),
            _ => None,
        }
    }

    /// Consume and return the inner SIP/SIPS URI, if any.
    pub fn into_sip(self) -> Option<SipUri> {
        match self {
            Uri::Sip(u) => Some(u),
            _ => None,
        }
    }

    /// Consume and return the inner tel: URI, if any.
    pub fn into_tel(self) -> Option<TelUri> {
        match self {
            Uri::Tel(u) => Some(u),
            _ => None,
        }
    }

    /// Consume and return the inner URN, if any.
    pub fn into_urn(self) -> Option<UrnUri> {
        match self {
            Uri::Urn(u) => Some(u),
            _ => None,
        }
    }

    /// Consume and return the URI text for an unrecognized scheme.
    pub fn into_other(self) -> Option<String> {
        match self {
            Uri::Other(o) => Some(o.raw),
            _ => None,
        }
    }

    /// The scheme of this URI, lowercase, `None` when the input had none.
    pub fn scheme(&self) -> Option<&str> {
        match self {
            Uri::Sip(u) => u
                .scheme()
                .map(Scheme::as_str),
            Uri::Tel(_) => Some("tel"),
            Uri::Urn(_) => Some("urn"),
            Uri::Other(o) => o.scheme(),
        }
    }

    /// Extract the user/subscriber identifier from the URI.
    ///
    /// - `sip:`/`sips:` → user part before `@`
    /// - `tel:` → phone number (the dialable identifier)
    /// - `urn:` / other → `None`
    pub fn user(&self) -> Option<&str> {
        match self {
            Uri::Sip(u) => u.user(),
            Uri::Tel(u) => u.number(),
            Uri::Urn(_) | Uri::Other(_) => None,
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
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_with_warnings(s).map(|parsed| parsed.value)
    }
}

impl Uri {
    /// Parse, rejecting any grammar breach as [`ParseError::NonConformant`].
    pub fn parse_strict(input: &str) -> Result<Self, ParseError> {
        Self::parse_with_warnings(input)?.into_strict()
    }

    /// Parse, reporting accepted grammar breaches beside the value.
    ///
    /// Accepts exactly what [`FromStr`] accepts: everything but empty input.
    /// Input without a scheme, or with one outside the RFC 3986 grammar such
    /// as a URI still wrapped in `<>`, is kept as [`Uri::Other`] with a
    /// warning.
    pub fn parse_with_warnings(s: &str) -> Result<Parsed<Self>, ParseError> {
        fn wrap<T>(parsed: Parsed<T>, variant: fn(T) -> Uri) -> Parsed<Uri> {
            Parsed {
                value: variant(parsed.value),
                warnings: parsed.warnings,
            }
        }

        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        let mut warnings = Warnings::new(s);
        match parse::split_scheme(s) {
            SchemeSplit::Named(scheme, _) if scheme.eq_ignore_ascii_case("tel") => {
                return Ok(wrap(TelUri::parse_with_warnings(s)?, Uri::Tel));
            }
            SchemeSplit::Named(scheme, _)
                if scheme.eq_ignore_ascii_case("sip") || scheme.eq_ignore_ascii_case("sips") =>
            {
                return Ok(wrap(SipUri::parse_with_warnings(s)?, Uri::Sip));
            }
            SchemeSplit::Named(scheme, _) if scheme.eq_ignore_ascii_case("urn") => {
                return Ok(wrap(UrnUri::parse_with_warnings(s)?, Uri::Urn));
            }
            SchemeSplit::Named(..) => {}
            SchemeSplit::Invalid => {
                warnings.push(Component::Scheme, WarningCode::InvalidScheme, s, 0);
            }
            SchemeSplit::Absent if s == "*" => {
                warnings.push(Component::Scheme, WarningCode::Wildcard, s, 0);
            }
            SchemeSplit::Absent => {
                warnings.push(Component::Scheme, WarningCode::MissingScheme, s, 0);
            }
        }
        Ok(warnings.finish(Uri::Other(OtherUri::new(s))))
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Uri::Sip(u) => write!(f, "{u}"),
            Uri::Tel(u) => write!(f, "{u}"),
            Uri::Urn(u) => write!(f, "{u}"),
            Uri::Other(o) => write!(f, "{o}"),
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
        assert_eq!(uri.scheme(), Some("http"));
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
    fn missing_scheme_is_other() {
        let parsed = Uri::parse_with_warnings("no-colon-here").unwrap();
        assert_eq!(
            parsed
                .value
                .as_other(),
            Some("no-colon-here")
        );
        assert_eq!(
            parsed
                .value
                .scheme(),
            None
        );
        assert_eq!(parsed.warnings[0].code, WarningCode::MissingScheme);
        assert!(""
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

    #[test]
    fn user_sip() {
        let uri: Uri = "sip:alice@example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), Some("alice"));
    }

    #[test]
    fn user_sip_no_user() {
        let uri: Uri = "sip:example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), None);
    }

    #[test]
    fn user_tel() {
        let uri: Uri = "tel:+15551234567"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), Some("+15551234567"));
    }

    #[test]
    fn user_urn() {
        let uri: Uri = "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), None);
    }

    #[test]
    fn user_other() {
        let uri: Uri = "http://example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), None);
    }
}
