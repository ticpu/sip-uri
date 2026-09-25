use std::fmt;
use std::str::FromStr;

use crate::error::ParseError;
use crate::parse::{self, percent_decode, SchemeSplit};
use crate::warning::{Component, Parsed, WarningCode, Warnings};

/// URN (Uniform Resource Name) per RFC 8141.
///
/// Represents `urn:NID:NSS` with optional resolution (`?+`), query (`?=`),
/// and fragment (`#`) components.
///
/// The NID is stored lowercase per RFC 8141 equivalence rules.
/// The NSS is stored as-is; percent-encoded hex digits are uppercased for
/// canonical comparison but the original octets are preserved (never decoded).
/// A missing NID or NSS is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UrnUri {
    nid: Option<String>,
    nss: Option<String>,
    r_component: Option<String>,
    q_component: Option<String>,
    f_component: Option<String>,
}

impl UrnUri {
    /// Create a new URN with the given NID and NSS.
    ///
    /// The NID is lowercased. No validation is performed on the builder path;
    /// use `FromStr` for validated parsing.
    pub fn new(nid: impl Into<String>, nss: impl Into<String>) -> Self {
        UrnUri {
            nid: Some(
                nid.into()
                    .to_ascii_lowercase(),
            ),
            nss: Some(nss.into()),
            r_component: None,
            q_component: None,
            f_component: None,
        }
    }

    /// Set the resolution component (`?+`).
    pub fn with_r_component(mut self, r: impl Into<String>) -> Self {
        self.r_component = Some(r.into());
        self
    }

    /// Set the query component (`?=`).
    pub fn with_q_component(mut self, q: impl Into<String>) -> Self {
        self.q_component = Some(q.into());
        self
    }

    /// Set the fragment component (`#`).
    pub fn with_f_component(mut self, f: impl Into<String>) -> Self {
        self.f_component = Some(f.into());
        self
    }

    /// The Namespace Identifier (always lowercase).
    pub fn nid(&self) -> Option<&str> {
        self.nid
            .as_deref()
    }

    /// The Namespace Specific String (as received, with hex uppercased).
    pub fn nss(&self) -> Option<&str> {
        self.nss
            .as_deref()
    }

    /// The resolution component, if present.
    pub fn r_component(&self) -> Option<&str> {
        self.r_component
            .as_deref()
    }

    /// The query component, if present.
    pub fn q_component(&self) -> Option<&str> {
        self.q_component
            .as_deref()
    }

    /// The fragment component, if present.
    pub fn f_component(&self) -> Option<&str> {
        self.f_component
            .as_deref()
    }

    /// The assigned-name portion (`urn:NID:NSS`) without optional components.
    pub fn assigned_name(&self) -> String {
        let mut s = String::from("urn:");
        push_assigned(&mut s, self.nid(), self.nss());
        s
    }
}

fn push_assigned(out: &mut String, nid: Option<&str>, nss: Option<&str>) {
    out.push_str(nid.unwrap_or_default());
    if let Some(nss) = nss {
        out.push(':');
        out.push_str(nss);
    }
}

/// RFC 8141: `NID = (alphanum) 0*30(ldh) (alphanum)` where `ldh = alphanum / "-"`.
fn is_valid_nid(nid: &str) -> bool {
    let bytes = nid.as_bytes();
    (2..=32).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
}

/// RFC 3986 pchar: `unreserved / pct-encoded / sub-delims / ":" / "@"`
fn is_pchar(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
        )
}

impl FromStr for UrnUri {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse_with_warnings(input).map(|parsed| parsed.value)
    }
}

impl UrnUri {
    /// Parse, rejecting any grammar breach as [`ParseError::NonConformant`].
    pub fn parse_strict(input: &str) -> Result<Self, ParseError> {
        Self::parse_with_warnings(input)?.into_strict()
    }

    /// Parse, reporting accepted grammar breaches beside the value.
    ///
    /// Accepts exactly what [`FromStr`] accepts: everything except empty
    /// input and a scheme other than `urn`.
    pub fn parse_with_warnings(input: &str) -> Result<Parsed<Self>, ParseError> {
        if input.is_empty() {
            return Err(ParseError::Empty);
        }
        let mut warnings = Warnings::new(input);

        let rest = match parse::split_scheme(input) {
            SchemeSplit::Named(s, rest) if s.eq_ignore_ascii_case("urn") => rest,
            SchemeSplit::Named(..) => return Err(ParseError::SchemeMismatch),
            SchemeSplit::Invalid => {
                warnings.push(Component::Scheme, WarningCode::InvalidScheme, input, 0);
                input
            }
            SchemeSplit::Absent => {
                warnings.push(Component::Scheme, WarningCode::MissingScheme, input, 0);
                input
            }
        };

        let (nid_str, after_nid) = match rest.split_once(':') {
            Some((nid, after)) => (nid, Some(after)),
            None => (rest, None),
        };
        let nid = if nid_str.is_empty() {
            warnings.push(Component::Nid, WarningCode::MissingNid, nid_str, 0);
            None
        } else {
            if !is_valid_nid(nid_str) {
                warnings.push(Component::Nid, WarningCode::InvalidNid, nid_str, 0);
            }
            Some(nid_str.to_ascii_lowercase())
        };

        let after_nid = after_nid.unwrap_or_default();

        // `#` appears in no component, so the first one starts the fragment.
        let (before_fragment, f_component) = match after_nid.split_once('#') {
            Some((before, frag)) => {
                warnings.charset(Component::FComponent, frag, is_rqf_char);
                (before, Some(frag.to_string()))
            }
            None => (after_nid, None),
        };

        // `?` is not a pchar: `?+` or `?=` ends the NSS, any other `?` stays in it.
        let rq_start = before_fragment
            .match_indices('?')
            .map(|(i, _)| i)
            .find(|&i| {
                matches!(
                    before_fragment
                        .as_bytes()
                        .get(i + 1),
                    Some(b'+' | b'=')
                )
            });
        let (nss_str, rq_str) = match rq_start {
            Some(q) => (&before_fragment[..q], Some(&before_fragment[q..])),
            None => (before_fragment, None),
        };

        let nss = if nss_str.is_empty() {
            warnings.push(Component::Nss, WarningCode::MissingNss, nss_str, 0);
            None
        } else {
            if nss_str.starts_with('/') {
                warnings.push(Component::Nss, WarningCode::InvalidChar, nss_str, 0);
            } else {
                warnings.charset(Component::Nss, nss_str, |b| is_pchar(b) || b == b'/');
            }
            Some(percent_decode(nss_str, |_| false))
        };

        let (r_component, q_component) = match rq_str {
            Some(rq) => parse_rq_components(rq),
            None => (None, None),
        };
        for (component, value) in [
            (Component::RComponent, r_component),
            (Component::QComponent, q_component),
        ] {
            if let Some(value) = value {
                if value.is_empty() {
                    warnings.push(component, WarningCode::EmptyComponent, value, 0);
                }
                warnings.charset(component, value, is_rqf_char);
            }
        }

        Ok(warnings.finish(UrnUri {
            nid,
            nss,
            r_component: r_component.map(str::to_string),
            q_component: q_component.map(str::to_string),
            f_component,
        }))
    }
}

/// RFC 8141: r-, q- and f-components are `*( pchar / "/" / "?" )`.
fn is_rqf_char(b: u8) -> bool {
    is_pchar(b) || matches!(b, b'/' | b'?')
}

/// Split `?+r` and/or `?=q`; the input starts with one of the two.
fn parse_rq_components(s: &str) -> (Option<&str>, Option<&str>) {
    if let Some(r) = s.strip_prefix("?+") {
        match r.split_once("?=") {
            Some((r, q)) => (Some(r), Some(q)),
            None => (Some(r), None),
        }
    } else {
        (None, s.strip_prefix("?="))
    }
}

impl fmt::Display for UrnUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut assigned = String::from("urn:");
        push_assigned(&mut assigned, self.nid(), self.nss());
        f.write_str(&assigned)?;
        if let Some(ref r) = self.r_component {
            write!(f, "?+{r}")?;
        }
        if let Some(ref q) = self.q_component {
            write!(f, "?={q}")?;
        }
        if let Some(ref frag) = self.f_component {
            write!(f, "#{frag}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_service_sos() {
        let urn: UrnUri = "urn:service:sos"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("service"));
        assert_eq!(urn.nss(), Some("sos"));
    }

    #[test]
    fn parse_service_sos_subtype() {
        let urn: UrnUri = "urn:service:sos.fire"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("service"));
        assert_eq!(urn.nss(), Some("sos.fire"));
    }

    #[test]
    fn parse_nena_service() {
        let urn: UrnUri = "urn:nena:service:sos"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("nena"));
        assert_eq!(urn.nss(), Some("service:sos"));
    }

    #[test]
    fn parse_nena_uid_callid() {
        let urn: UrnUri = "urn:nena:callid:20250101120000001TEST001:bcf1.ng911.example.com"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("nena"));
        assert_eq!(
            urn.nss()
                .unwrap(),
            "callid:20250101120000001TEST001:bcf1.ng911.example.com"
        );
    }

    #[test]
    fn parse_emergency_incidentid() {
        let urn: UrnUri =
            "urn:emergency:incidentid:f1e2d3c4b5a6f7e8d9c0b1a2f3e4d5c6:bcf.ng911.example.com"
                .parse()
                .unwrap();
        assert_eq!(urn.nid(), Some("emergency"));
        assert!(urn
            .nss()
            .unwrap()
            .starts_with("incidentid:"));
    }

    #[test]
    fn parse_gsma_imei() {
        let urn: UrnUri = "urn:gsma:imei:35625207-210812-0"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("gsma"));
        assert_eq!(urn.nss(), Some("imei:35625207-210812-0"));
    }

    #[test]
    fn parse_urn7_3gpp() {
        let urn: UrnUri = "urn:urn-7:3gpp-service.ims.icsi.mmtel"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("urn-7"));
        assert_eq!(urn.nss(), Some("3gpp-service.ims.icsi.mmtel"));
    }

    #[test]
    fn parse_uuid() {
        let urn: UrnUri = "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("uuid"));
        assert_eq!(urn.nss(), Some("f81d4fae-7dec-11d0-a765-00a0c91e6bf6"));
    }

    #[test]
    fn parse_case_insensitive_scheme() {
        let urn: UrnUri = "URN:service:sos"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("service"));
    }

    #[test]
    fn nid_case_insensitive() {
        let urn: UrnUri = "urn:SERVICE:sos"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("service"));
    }

    #[test]
    fn nss_percent_encoding_uppercased() {
        let urn: UrnUri = "urn:example:foo%2fbar"
            .parse()
            .unwrap();
        assert_eq!(urn.nss(), Some("foo%2Fbar"));
    }

    #[test]
    fn display_roundtrip() {
        let input = "urn:service:sos.police";
        let urn: UrnUri = input
            .parse()
            .unwrap();
        assert_eq!(urn.to_string(), input);
    }

    #[test]
    fn display_roundtrip_nena_callid() {
        let input = "urn:nena:callid:abc123:host.example.com";
        let urn: UrnUri = input
            .parse()
            .unwrap();
        assert_eq!(urn.to_string(), input);
    }

    #[test]
    fn with_rq_components() {
        let urn: UrnUri = "urn:example:foo?+resolve?=query#frag"
            .parse()
            .unwrap();
        assert_eq!(urn.nss(), Some("foo"));
        assert_eq!(urn.r_component(), Some("resolve"));
        assert_eq!(urn.q_component(), Some("query"));
        assert_eq!(urn.f_component(), Some("frag"));
        assert_eq!(urn.to_string(), "urn:example:foo?+resolve?=query#frag");
    }

    #[test]
    fn with_r_component_only() {
        let urn: UrnUri = "urn:example:foo?+resolve"
            .parse()
            .unwrap();
        assert_eq!(urn.r_component(), Some("resolve"));
        assert_eq!(urn.q_component(), None);
    }

    #[test]
    fn with_q_component_only() {
        let urn: UrnUri = "urn:example:foo?=query"
            .parse()
            .unwrap();
        assert_eq!(urn.r_component(), None);
        assert_eq!(urn.q_component(), Some("query"));
    }

    #[test]
    fn with_fragment_only() {
        let urn: UrnUri = "urn:example:foo#section1"
            .parse()
            .unwrap();
        assert_eq!(urn.f_component(), Some("section1"));
        assert_eq!(urn.r_component(), None);
        assert_eq!(urn.q_component(), None);
    }

    #[test]
    fn assigned_name() {
        let urn: UrnUri = "urn:service:sos?+r?=q#f"
            .parse()
            .unwrap();
        assert_eq!(urn.assigned_name(), "urn:service:sos");
    }

    #[test]
    fn nss_with_slashes() {
        let urn: UrnUri = "urn:example:a/b/c"
            .parse()
            .unwrap();
        assert_eq!(urn.nss(), Some("a/b/c"));
    }

    #[test]
    fn nss_with_colons() {
        let urn: UrnUri = "urn:example:a:b:c"
            .parse()
            .unwrap();
        assert_eq!(urn.nss(), Some("a:b:c"));
    }

    #[test]
    fn builder() {
        let urn = UrnUri::new("service", "sos.fire");
        assert_eq!(urn.to_string(), "urn:service:sos.fire");
    }

    #[test]
    fn builder_with_components() {
        let urn = UrnUri::new("example", "resource")
            .with_r_component("resolve")
            .with_q_component("query")
            .with_f_component("section");
        assert_eq!(
            urn.to_string(),
            "urn:example:resource?+resolve?=query#section"
        );
    }

    // Negative tests

    #[test]
    fn missing_scheme() {
        assert!("service:sos"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn wrong_scheme() {
        assert!("http:service:sos"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn nonconformant_nid_and_nss_warn() {
        let long_nid = format!("urn:{}:foo", "a".repeat(33));
        for (input, code) in [
            ("urn:x:foo", WarningCode::InvalidNid),
            (long_nid.as_str(), WarningCode::InvalidNid),
            ("urn:-ab:foo", WarningCode::InvalidNid),
            ("urn:ab-:foo", WarningCode::InvalidNid),
            ("urn:example:", WarningCode::MissingNss),
            ("urn:example:/foo", WarningCode::InvalidChar),
            ("urn:example:foo?x", WarningCode::InvalidChar),
        ] {
            let parsed = UrnUri::parse_with_warnings(input).unwrap();
            assert_eq!(
                parsed
                    .warnings
                    .iter()
                    .map(|w| w.code)
                    .collect::<Vec<_>>(),
                [code],
                "{input}"
            );
            assert_eq!(
                parsed
                    .value
                    .to_string()
                    .parse::<UrnUri>()
                    .unwrap(),
                parsed.value,
                "{input}"
            );
        }
    }

    #[test]
    fn service_sos_with_port_in_to_header() {
        // Seen in production: `<urn:service:sos:5060>` in To header
        // The `:5060` is part of the NSS (not a port), and parses fine
        let urn: UrnUri = "urn:service:sos:5060"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("service"));
        assert_eq!(urn.nss(), Some("sos:5060"));
    }

    #[test]
    fn vendor_provider_id() {
        let urn: UrnUri = "urn:example:ng911:lsp:provider1"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("example"));
        assert_eq!(urn.nss(), Some("ng911:lsp:provider1"));
    }

    #[test]
    fn nena_service_responder_police() {
        let urn: UrnUri = "urn:nena:service:responder.police"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), Some("nena"));
        assert_eq!(urn.nss(), Some("service:responder.police"));
    }
}
