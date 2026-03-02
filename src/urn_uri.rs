use std::fmt;
use std::str::FromStr;

use crate::error::ParseUrnError;

/// URN (Uniform Resource Name) per RFC 8141.
///
/// Represents `urn:NID:NSS` with optional resolution (`?+`), query (`?=`),
/// and fragment (`#`) components.
///
/// The NID is stored lowercase per RFC 8141 equivalence rules.
/// The NSS is stored as-is; percent-encoded hex digits are uppercased for
/// canonical comparison but the original octets are preserved (never decoded).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UrnUri {
    nid: String,
    nss: String,
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
            nid: nid
                .into()
                .to_ascii_lowercase(),
            nss: nss.into(),
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
    pub fn nid(&self) -> &str {
        &self.nid
    }

    /// The Namespace Specific String (as received, with hex uppercased).
    pub fn nss(&self) -> &str {
        &self.nss
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
        format!("urn:{}:{}", self.nid, self.nss)
    }
}

/// RFC 8141: `NID = (alphanum) 0*30(ldh) (alphanum)` where `ldh = alphanum / "-"`.
/// Length 2-32, first and last char alphanumeric, interior alphanum or hyphen.
fn validate_nid(nid: &str) -> Result<(), String> {
    let bytes = nid.as_bytes();
    let len = bytes.len();

    if len < 2 {
        return Err("NID must be at least 2 characters".into());
    }
    if len > 32 {
        return Err("NID must be at most 32 characters".into());
    }
    if !bytes[0].is_ascii_alphanumeric() {
        return Err("NID must start with alphanumeric character".into());
    }
    if !bytes[len - 1].is_ascii_alphanumeric() {
        return Err("NID must end with alphanumeric character".into());
    }
    for &b in &bytes[1..len - 1] {
        if !b.is_ascii_alphanumeric() && b != b'-' {
            return Err(format!("NID contains invalid character '{}'", b as char));
        }
    }

    Ok(())
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

/// Validate NSS: `pchar *(pchar / "/")`, with percent-encoded sequences.
fn validate_nss(nss: &str) -> Result<(), String> {
    if nss.is_empty() {
        return Err("NSS must not be empty".into());
    }

    let bytes = nss.as_bytes();
    let mut i = 0;

    // First char must be pchar (not "/")
    if bytes[0] == b'/' {
        return Err("NSS must not start with '/'".into());
    }

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return Err(format!("invalid percent-encoding at position {i}"));
            }
            i += 3;
        } else if is_pchar(bytes[i]) || bytes[i] == b'/' {
            i += 1;
        } else {
            return Err(format!(
                "invalid character '{}' in NSS at position {i}",
                bytes[i] as char
            ));
        }
    }

    Ok(())
}

/// Uppercase hex digits in percent-encoded sequences for canonical comparison.
fn canonize_percent_encoding(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            out.push('%');
            out.push((bytes[i + 1] as char).to_ascii_uppercase());
            out.push((bytes[i + 2] as char).to_ascii_uppercase());
            i += 3;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

impl FromStr for UrnUri {
    type Err = ParseUrnError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let err = |msg: &str| ParseUrnError(msg.to_string());

        // Strip "urn:" scheme (case-insensitive)
        let rest = {
            let colon = input
                .find(':')
                .ok_or_else(|| err("missing scheme"))?;
            if !input[..colon].eq_ignore_ascii_case("urn") {
                return Err(err("scheme must be 'urn'"));
            }
            &input[colon + 1..]
        };

        // Extract NID (up to next ':')
        let nid_end = rest
            .find(':')
            .ok_or_else(|| err("missing ':' after NID"))?;
        let nid_str = &rest[..nid_end];
        validate_nid(nid_str).map_err(|e| err(&e))?;

        let after_nid = &rest[nid_end + 1..];

        // Split off fragment (#) first -- '#' is not valid in NSS or rq-components
        // delimiters, but IS valid inside r/q/f components. However, the outermost
        // '#' starts the fragment.
        let (before_fragment, f_component) = if let Some(hash) = after_nid.rfind('#') {
            let frag = &after_nid[hash + 1..];
            (&after_nid[..hash], Some(frag.to_string()))
        } else {
            (after_nid, None)
        };

        // Find the end of NSS: first '?' that is part of rq-components.
        // '?' is NOT a pchar, so the first '?' ends the NSS.
        let (nss_str, rq_str) = if let Some(q) = before_fragment.find('?') {
            (&before_fragment[..q], Some(&before_fragment[q..]))
        } else {
            (before_fragment, None)
        };

        // Validate and canonize NSS
        validate_nss(nss_str).map_err(|e| err(&e))?;
        let nss = canonize_percent_encoding(nss_str);

        // Parse rq-components: [ "?+" r-component ] [ "?=" q-component ]
        let (r_component, q_component) = if let Some(rq) = rq_str {
            parse_rq_components(rq).map_err(|e| err(&e))?
        } else {
            (None, None)
        };

        Ok(UrnUri {
            nid: nid_str.to_ascii_lowercase(),
            nss,
            r_component,
            q_component,
            f_component,
        })
    }
}

/// Parse `?+r_component` and/or `?=q_component` from the rq portion.
///
/// Input starts with `?`. Per RFC 8141:
/// - `?+` introduces the r-component
/// - `?=` introduces the q-component
/// - r-component comes before q-component if both present
fn parse_rq_components(s: &str) -> Result<(Option<String>, Option<String>), String> {
    debug_assert!(s.starts_with('?'));

    if s.len() < 2 {
        return Err("unexpected '?' without '+' or '=' in URN".into());
    }

    match s.as_bytes()[1] {
        b'+' => {
            // r-component: extends to "?=" or end
            let r_start = 2;
            if let Some(qe) = s[r_start..].find("?=") {
                let r = &s[r_start..r_start + qe];
                let q = &s[r_start + qe + 2..];
                Ok((Some(r.to_string()), Some(q.to_string())))
            } else {
                Ok((Some(s[r_start..].to_string()), None))
            }
        }
        b'=' => {
            // q-component only (no r-component)
            Ok((None, Some(s[2..].to_string())))
        }
        _ => Err(format!(
            "unexpected '?{}' in URN (expected '?+' or '?=')",
            s.as_bytes()[1] as char
        )),
    }
}

impl fmt::Display for UrnUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "urn:{}:{}", self.nid, self.nss)?;
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
        assert_eq!(urn.nid(), "service");
        assert_eq!(urn.nss(), "sos");
    }

    #[test]
    fn parse_service_sos_subtype() {
        let urn: UrnUri = "urn:service:sos.fire"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "service");
        assert_eq!(urn.nss(), "sos.fire");
    }

    #[test]
    fn parse_nena_service() {
        let urn: UrnUri = "urn:nena:service:sos"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "nena");
        assert_eq!(urn.nss(), "service:sos");
    }

    #[test]
    fn parse_nena_uid_callid() {
        let urn: UrnUri = "urn:nena:callid:20250101120000001TEST001:bcf1.ng911.example.com"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "nena");
        assert_eq!(
            urn.nss(),
            "callid:20250101120000001TEST001:bcf1.ng911.example.com"
        );
    }

    #[test]
    fn parse_emergency_incidentid() {
        let urn: UrnUri =
            "urn:emergency:incidentid:f1e2d3c4b5a6f7e8d9c0b1a2f3e4d5c6:bcf.ng911.example.com"
                .parse()
                .unwrap();
        assert_eq!(urn.nid(), "emergency");
        assert!(urn
            .nss()
            .starts_with("incidentid:"));
    }

    #[test]
    fn parse_gsma_imei() {
        let urn: UrnUri = "urn:gsma:imei:35625207-210812-0"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "gsma");
        assert_eq!(urn.nss(), "imei:35625207-210812-0");
    }

    #[test]
    fn parse_urn7_3gpp() {
        let urn: UrnUri = "urn:urn-7:3gpp-service.ims.icsi.mmtel"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "urn-7");
        assert_eq!(urn.nss(), "3gpp-service.ims.icsi.mmtel");
    }

    #[test]
    fn parse_uuid() {
        let urn: UrnUri = "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "uuid");
        assert_eq!(urn.nss(), "f81d4fae-7dec-11d0-a765-00a0c91e6bf6");
    }

    #[test]
    fn parse_case_insensitive_scheme() {
        let urn: UrnUri = "URN:service:sos"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "service");
    }

    #[test]
    fn nid_case_insensitive() {
        let urn: UrnUri = "urn:SERVICE:sos"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "service");
    }

    #[test]
    fn nss_percent_encoding_uppercased() {
        let urn: UrnUri = "urn:example:foo%2fbar"
            .parse()
            .unwrap();
        assert_eq!(urn.nss(), "foo%2Fbar");
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
        assert_eq!(urn.nss(), "foo");
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
        assert_eq!(urn.nss(), "a/b/c");
    }

    #[test]
    fn nss_with_colons() {
        let urn: UrnUri = "urn:example:a:b:c"
            .parse()
            .unwrap();
        assert_eq!(urn.nss(), "a:b:c");
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
    fn nid_too_short() {
        assert!("urn:x:foo"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn nid_too_long() {
        let long_nid = "a".repeat(33);
        assert!(format!("urn:{long_nid}:foo")
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn nid_starts_with_hyphen() {
        assert!("urn:-ab:foo"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn nid_ends_with_hyphen() {
        assert!("urn:ab-:foo"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn empty_nss() {
        assert!("urn:example:"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn nss_starts_with_slash() {
        assert!("urn:example:/foo"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn invalid_rq_delimiter() {
        assert!("urn:example:foo?x"
            .parse::<UrnUri>()
            .is_err());
    }

    #[test]
    fn service_sos_with_port_in_to_header() {
        // Seen in production: `<urn:service:sos:5060>` in To header
        // The `:5060` is part of the NSS (not a port), and parses fine
        let urn: UrnUri = "urn:service:sos:5060"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "service");
        assert_eq!(urn.nss(), "sos:5060");
    }

    #[test]
    fn vendor_provider_id() {
        let urn: UrnUri = "urn:example:ng911:lsp:provider1"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "example");
        assert_eq!(urn.nss(), "ng911:lsp:provider1");
    }

    #[test]
    fn nena_service_responder_police() {
        let urn: UrnUri = "urn:nena:service:responder.police"
            .parse()
            .unwrap();
        assert_eq!(urn.nid(), "nena");
        assert_eq!(urn.nss(), "service:responder.police");
    }
}
