use std::fmt;

use crate::canon;
use crate::sip_uri::Scheme;
use crate::sip_uri::SipUri;
use crate::tel_uri::TelUri;
use crate::urn_uri::UrnUri;

/// A URI: SIP/SIPS, tel, URN, or an opaque URI with an unrecognized scheme.
///
/// The `Other` variant keeps text without a structure of its own (e.g. `http:`,
/// `https:`, `data:`, or text without a scheme), so header values like
/// `Call-Info` round-trip without rejecting non-SIP URIs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
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

/// Text kept as sent except for its scheme, which is lowercased, and the
/// bytes [`OtherUri::new`] escapes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OtherUri {
    raw: String,
    scheme_end: Option<usize>,
}

impl OtherUri {
    /// `scheme:rest` with the scheme lowercased, or `rest` alone without one.
    ///
    /// In `rest`, control bytes, space, `<`, `>`, `"` and every byte of a
    /// non-ASCII character become uppercase `%XX`; nothing is decoded, and
    /// every other byte, `%` included, is kept as sent.
    ///
    /// `None` when the scheme is outside the RFC 3986 grammar or is `sip`,
    /// `sips`, `tel` or `urn`, which have their own types, or when there is
    /// neither scheme nor text.
    ///
    /// ```
    /// use sip_uri_types::OtherUri;
    ///
    /// let uri = OtherUri::new(Some("HTTPS"), "//example.com/a b").unwrap();
    /// assert_eq!(uri.as_str(), "https://example.com/a%20b");
    /// ```
    pub fn new(scheme: Option<&str>, rest: &str) -> Option<Self> {
        match scheme {
            None if rest.is_empty() => None,
            None => Some(OtherUri {
                raw: canon::canonize_other(rest),
                scheme_end: None,
            }),
            Some(s) if !canon::is_scheme(s) || canon::is_known_scheme(s) => None,
            Some(s) => Some(OtherUri {
                raw: format!("{}:{}", s.to_ascii_lowercase(), canon::canonize_other(rest)),
                scheme_end: Some(s.len()),
            }),
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

    /// The text after the scheme's `:`, or the whole text without a scheme.
    pub fn rest(&self) -> &str {
        let start = self
            .scheme_end
            .map_or(0, |end| end + 1);
        &self.raw[start..]
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
    fn other_uri_refuses_known_and_invalid_schemes() {
        assert_eq!(OtherUri::new(Some("SIP"), "alice@example.com"), None);
        assert_eq!(OtherUri::new(Some("<sip"), "alice@example.com"), None);
        let other = OtherUri::new(Some("HTTPS"), "//example.com").unwrap();
        assert_eq!(other.as_str(), "https://example.com");
        assert_eq!(other.scheme(), Some("https"));
        let bare = OtherUri::new(None, "*").unwrap();
        assert_eq!(bare.scheme(), None);
        assert_eq!(bare.as_str(), "*");
        assert_eq!(bare.rest(), "*");
        assert_eq!(other.rest(), "//example.com");
        assert_eq!(OtherUri::new(None, ""), None);
    }

    #[test]
    fn other_uri_escapes_what_breaks_a_header_line() {
        let other = OtherUri::new(Some("http"), "//x\r\nVia: <y> \"é\" %zz").unwrap();
        assert_eq!(
            other.as_str(),
            "http://x%0D%0AVia:%20%3Cy%3E%20%22%C3%A9%22%20%zz"
        );
        assert_eq!(OtherUri::new(Some("http"), other.rest()), Some(other));
        assert_eq!(
            OtherUri::new(None, "a\u{7f}b")
                .unwrap()
                .as_str(),
            "a%7Fb"
        );
    }
}
