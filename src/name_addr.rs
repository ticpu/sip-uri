use std::fmt;
use std::str::FromStr;

use crate::error::ParseNameAddrError;
use crate::sip_uri::SipUri;
use crate::tel_uri::TelUri;
use crate::uri::Uri;
use crate::urn_uri::UrnUri;

/// A SIP name-addr: optional display name with a URI.
///
/// Parses the following forms:
///
/// - `"Display Name" <sip:user@host>`
/// - `<sip:user@host>`
/// - `sip:user@host` (bare URI, no display name)
///
/// **Deprecated since 0.2.0, will be removed in 0.3.0.**
///
/// The `name-addr` production (RFC 3261 §25.1) appears inside SIP header
/// fields like `From`, `To`, `Contact`, and `Refer-To`, where it is
/// followed by header-level parameters (`;tag=`, `;expires=`, etc.).
/// This type rejects those parameters, so it cannot round-trip real SIP
/// header values.
///
/// # Migration
///
/// - **Full SIP header parsing**: use
///   [`SipHeaderAddr`](https://docs.rs/sip-header/latest/sip_header/struct.SipHeaderAddr.html)
///   from the [`sip-header`](https://crates.io/crates/sip-header)
///   crate, which handles display names, URIs, and header-level parameters.
/// - **URI only**: parse directly with [`Uri`](crate::Uri).
#[deprecated(
    since = "0.2.0",
    note = "name-addr is header-level grammar; use sip_header::SipHeaderAddr or parse the URI with sip_uri::Uri directly"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct NameAddr {
    display_name: Option<String>,
    uri: Uri,
}

#[allow(deprecated)]
impl NameAddr {
    /// Create a new NameAddr with the given URI and no display name.
    pub fn new(uri: Uri) -> Self {
        NameAddr {
            display_name: None,
            uri,
        }
    }

    /// Set the display name.
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// The display name, if present.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name
            .as_deref()
    }

    /// The URI.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// If the URI is a SIP/SIPS URI, return a reference to it.
    pub fn sip_uri(&self) -> Option<&SipUri> {
        self.uri
            .as_sip()
    }

    /// If the URI is a tel: URI, return a reference to it.
    pub fn tel_uri(&self) -> Option<&TelUri> {
        self.uri
            .as_tel()
    }

    /// If the URI is a URN, return a reference to it.
    pub fn urn_uri(&self) -> Option<&UrnUri> {
        self.uri
            .as_urn()
    }
}

#[allow(deprecated)]
impl FromStr for NameAddr {
    type Err = ParseNameAddrError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let err = |msg: &str| ParseNameAddrError(msg.to_string());
        let s = input.trim();

        if s.is_empty() {
            return Err(err("empty input"));
        }

        // Case 1: quoted display name followed by <URI>
        if s.starts_with('"') {
            let (display_name, rest) = parse_quoted_string(s).map_err(|e| err(&e))?;
            let rest = rest.trim_start();
            let (uri_str, trailing) = extract_angle_uri(rest)
                .ok_or_else(|| err("expected '<URI>' after quoted display name"))?;
            reject_trailing(trailing)?;
            let uri: Uri = uri_str.parse()?;
            let display_name = if display_name.is_empty() {
                None
            } else {
                Some(display_name)
            };
            return Ok(NameAddr { display_name, uri });
        }

        // Case 2: <URI> without display name
        if s.starts_with('<') {
            let (uri_str, trailing) = extract_angle_uri(s).ok_or_else(|| err("unclosed '<'"))?;
            reject_trailing(trailing)?;
            let uri: Uri = uri_str.parse()?;
            return Ok(NameAddr {
                display_name: None,
                uri,
            });
        }

        // Case 3: unquoted display name followed by <URI>
        // or bare URI without angle brackets
        if let Some(angle_start) = s.find('<') {
            let display_name = s[..angle_start].trim();
            let display_name = if display_name.is_empty() {
                None
            } else {
                Some(display_name.to_string())
            };
            let (uri_str, trailing) =
                extract_angle_uri(&s[angle_start..]).ok_or_else(|| err("unclosed '<'"))?;
            reject_trailing(trailing)?;
            let uri: Uri = uri_str.parse()?;
            return Ok(NameAddr { display_name, uri });
        }

        // Case 4: bare URI (no angle brackets, no display name)
        let uri: Uri = s.parse()?;
        Ok(NameAddr {
            display_name: None,
            uri,
        })
    }
}

/// Reject any non-whitespace content after `>`.
///
/// Header-level parameters (`;tag=`, `;serviceurn=`, etc.) are part of the
/// SIP header field grammar, not `name-addr`. Callers must split those off
/// before parsing.
fn reject_trailing(s: &str) -> Result<(), ParseNameAddrError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Ok(())
    } else {
        Err(ParseNameAddrError(format!(
            "trailing content after '>': \"{trimmed}\" \
             (header-level parameters belong in SIP header parsing, not name-addr)"
        )))
    }
}

/// Extract the URI from between `<` and `>`, returning (uri, rest_after_`>`).
fn extract_angle_uri(s: &str) -> Option<(&str, &str)> {
    let s = s.strip_prefix('<')?;
    let end = s.find('>')?;
    Some((&s[..end], &s[end + 1..]))
}

/// Parse a quoted string and return (unescaped content, rest of input after closing quote).
fn parse_quoted_string(s: &str) -> Result<(String, &str), String> {
    if !s.starts_with('"') {
        return Err("expected opening quote".into());
    }

    let mut result = String::new();
    let mut chars = s[1..].char_indices();

    while let Some((i, c)) = chars.next() {
        match c {
            '"' => {
                // +2: skip opening quote + position after closing quote
                return Ok((result, &s[i + 2..]));
            }
            '\\' => {
                let (_, escaped) = chars
                    .next()
                    .ok_or("unterminated escape in quoted string")?;
                result.push(escaped);
            }
            _ => {
                result.push(c);
            }
        }
    }

    Err("unterminated quoted string".into())
}

#[allow(deprecated)]
impl fmt::Display for NameAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self
            .display_name
            .as_deref()
        {
            Some(name) if !name.is_empty() => {
                if needs_quoting(name) {
                    write!(f, "\"{}\" ", escape_display_name(name))?;
                } else {
                    write!(f, "{name} ")?;
                }
                write!(f, "<{}>", self.uri)
            }
            _ => {
                write!(f, "<{}>", self.uri)
            }
        }
    }
}

/// Check if a display name needs quoting.
///
/// Needs quoting if it contains special chars or whitespace (since
/// unquoted tokens can't contain spaces per the SIP grammar).
fn needs_quoting(name: &str) -> bool {
    name.bytes()
        .any(|b| {
            matches!(
                b,
                b'"' | b'\\' | b'<' | b'>' | b',' | b';' | b':' | b'@' | b' ' | b'\t'
            )
        })
}

/// Escape a display name for use within double quotes.
fn escape_display_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn parse_quoted_display_name() {
        let na: NameAddr =
            r#""EXAMPLE CO" <sip:+15551234567;cpc=emergency;oli=0@198.51.100.1;user=phone>"#
                .parse()
                .unwrap();
        assert_eq!(na.display_name(), Some("EXAMPLE CO"));
        let sip = na
            .sip_uri()
            .unwrap();
        assert_eq!(sip.user(), Some("+15551234567"));
        assert_eq!(
            sip.user_params(),
            &[
                ("cpc".into(), Some("emergency".into())),
                ("oli".into(), Some("0".into())),
            ]
        );
        assert_eq!(sip.param("user"), Some(&Some("phone".into())));
    }

    #[test]
    fn parse_angle_brackets_no_name() {
        let na: NameAddr = "<sip:1305@pbx.example.com;user=phone>"
            .parse()
            .unwrap();
        assert_eq!(na.display_name(), None);
        assert!(na
            .sip_uri()
            .is_some());
    }

    #[test]
    fn reject_trailing_params_after_angle_bracket() {
        let cases = [
            "<sip:user@example.com>;tag=abc123",
            "<sip:user@example.com>;expires=3600;foo=bar",
            "<sip:user@example.com> trailing",
        ];
        for input in cases {
            assert!(
                input
                    .parse::<NameAddr>()
                    .is_err(),
                "should reject trailing content: {input}",
            );
        }
    }

    #[test]
    fn reject_trailing_params_after_quoted_name() {
        assert!(r#""Alice" <sip:alice@example.com>;expires=3600"#
            .parse::<NameAddr>()
            .is_err());
    }

    #[test]
    fn reject_trailing_params_after_unquoted_name() {
        assert!("Alice <sip:alice@example.com>;tag=xyz"
            .parse::<NameAddr>()
            .is_err());
    }

    #[test]
    fn parse_bare_uri() {
        let na: NameAddr = "sip:alice@example.com"
            .parse()
            .unwrap();
        assert_eq!(na.display_name(), None);
        assert!(na
            .sip_uri()
            .is_some());
    }

    #[test]
    fn parse_tel_in_angle_brackets() {
        let na: NameAddr = "<tel:+15551234567;cpc=emergency>"
            .parse()
            .unwrap();
        assert_eq!(na.display_name(), None);
        let tel = na
            .tel_uri()
            .unwrap();
        assert_eq!(tel.number(), "+15551234567");
    }

    #[test]
    fn parse_unquoted_display_name() {
        let na: NameAddr = "Alice <sip:alice@example.com>"
            .parse()
            .unwrap();
        assert_eq!(na.display_name(), Some("Alice"));
    }

    #[test]
    fn parse_escaped_quotes_in_display_name() {
        let na: NameAddr = r#""Say \"Hello\"" <sip:u@h>"#
            .parse()
            .unwrap();
        assert_eq!(na.display_name(), Some(r#"Say "Hello""#));
    }

    #[test]
    fn display_roundtrip_with_name() {
        let na: NameAddr = r#""EXAMPLE CO" <sip:+15551234567@198.51.100.1;user=phone>"#
            .parse()
            .unwrap();
        assert_eq!(
            na.to_string(),
            r#""EXAMPLE CO" <sip:+15551234567@198.51.100.1;user=phone>"#
        );
    }

    #[test]
    fn display_no_name() {
        let na: NameAddr = "<sip:alice@example.com>"
            .parse()
            .unwrap();
        assert_eq!(na.to_string(), "<sip:alice@example.com>");
    }

    #[test]
    fn builder() {
        let uri: Uri = "sip:alice@example.com"
            .parse()
            .unwrap();
        let na = NameAddr::new(uri).with_display_name("Alice");
        assert_eq!(na.to_string(), "Alice <sip:alice@example.com>");
    }

    #[test]
    fn empty_input_fails() {
        assert!(""
            .parse::<NameAddr>()
            .is_err());
    }
}
