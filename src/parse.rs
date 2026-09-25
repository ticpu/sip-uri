use std::borrow::Cow;

/// RFC 3261 §25: `unreserved = alphanum / mark`
/// `mark = "-" / "_" / "." / "!" / "~" / "*" / "'" / "(" / ")"`
pub(crate) fn is_unreserved(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
        )
}

/// RFC 3261 §25: `user-unreserved = "&" / "=" / "+" / "$" / "," / ";" / "?" / "/"`
pub(crate) fn is_user_unreserved(c: u8) -> bool {
    matches!(c, b'&' | b'=' | b'+' | b'$' | b',' | b';' | b'?' | b'/')
}

/// RFC 3261 §25: `param-unreserved = "[" / "]" / "/" / ":" / "&" / "+" / "$"`
///
/// Extended with `@` and `,` which appear in real-world SIP URIs
/// (e.g., sofia-sip torture tests) despite not being in the strict ABNF.
pub(crate) fn is_param_unreserved(c: u8) -> bool {
    matches!(
        c,
        b'[' | b']' | b'/' | b':' | b'&' | b'+' | b'$' | b'@' | b','
    )
}

/// RFC 3261 §25: `hnv-unreserved = "[" / "]" / "/" / "?" / ":" / "+" / "$"`
pub(crate) fn is_hnv_unreserved(c: u8) -> bool {
    matches!(c, b'[' | b']' | b'/' | b'?' | b':' | b'+' | b'$')
}

/// Characters allowed unescaped in the SIP user component:
/// unreserved + user-unreserved
pub(crate) fn is_user_char(c: u8) -> bool {
    is_unreserved(c) || is_user_unreserved(c)
}

/// Characters allowed unescaped in the password component:
/// unreserved + "&" / "=" / "+" / "$" / ","
pub(crate) fn is_password_char(c: u8) -> bool {
    is_unreserved(c) || matches!(c, b'&' | b'=' | b'+' | b'$' | b',')
}

/// Characters allowed unescaped in URI parameter names/values:
/// unreserved + param-unreserved
pub(crate) fn is_paramchar(c: u8) -> bool {
    is_unreserved(c) || is_param_unreserved(c)
}

/// `paramchar` without the `@` and `,` extension: RFC 3261 §25 as written.
pub(crate) fn is_param_strict(c: u8) -> bool {
    is_paramchar(c) && !matches!(c, b'@' | b',')
}

/// RFC 3966: `pname = 1*( alphanum / "-" )`
pub(crate) fn is_tel_pname_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-'
}

/// RFC 3966: `paramchar = param-unreserved / unreserved / pct-encoded`, whose
/// `param-unreserved` matches RFC 3261's without our extension.
pub(crate) fn is_tel_paramchar(c: u8) -> bool {
    is_param_strict(c)
}

/// Characters allowed unescaped in header names/values:
/// unreserved + hnv-unreserved
pub(crate) fn is_hnv_char(c: u8) -> bool {
    is_unreserved(c) || is_hnv_unreserved(c)
}

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// Percent-decode into bytes, applying a component-specific filter.
///
/// Only decodes `%XX` sequences where the decoded byte satisfies `allow_decoded`.
/// Other sequences stay encoded with hex normalized to uppercase. A `%` not
/// followed by two hex digits is copied verbatim.
fn percent_decode_bytes(input: &str, allow_decoded: impl Fn(u8) -> bool) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if let Some(decoded) = escape_at(bytes, i) {
            if allow_decoded(decoded) {
                out.push(decoded);
            } else {
                out.push(b'%');
                out.push(bytes[i + 1].to_ascii_uppercase());
                out.push(bytes[i + 2].to_ascii_uppercase());
            }
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }

    out
}

/// The octet escaped by a well-formed `%XX` starting at `i`.
fn escape_at(bytes: &[u8], i: usize) -> Option<u8> {
    if bytes.get(i) != Some(&b'%') {
        return None;
    }
    let hi = hex_digit(*bytes.get(i + 1)?)?;
    let lo = hex_digit(*bytes.get(i + 2)?)?;
    Some((hi << 4) | lo)
}

/// Percent-decode a string, applying a component-specific filter.
///
/// Only decodes `%XX` sequences to an ASCII byte satisfying `allow_decoded`.
/// Characters that are reserved in this component stay encoded.
/// Hex digits in percent-encoding are normalized to uppercase.
pub(crate) fn percent_decode(input: &str, allow_decoded: fn(u8) -> bool) -> String {
    let out = percent_decode_bytes(input, |b| b.is_ascii() && allow_decoded(b));

    // SAFETY: only ASCII bytes are decoded; everything else is copied from a
    // &str, so the output is valid UTF-8.
    unsafe { String::from_utf8_unchecked(out) }
}

/// Decode every `%XX` in a SIP user part to its octet.
///
/// The input is the encoded `user` production as it appears before `@`,
/// user-params included: `%2B1555` from FreeSWITCH's `sip_req_user`, or the
/// canonical form [`crate::SipUri::user`] holds. The result is the logical
/// value, not a URI component: `%3B` and a literal `;` both come out as `;`,
/// so a user-params split is lost, and the bytes need not be UTF-8. A `%` not
/// followed by two hex digits is copied verbatim.
///
/// Returns [`Cow::Borrowed`] when the input contains no `%`.
///
/// ```
/// use sip_uri::decode_user;
///
/// let decoded = decode_user("%2B15551234567");
/// assert_eq!(String::from_utf8_lossy(&decoded), "+15551234567");
/// ```
pub fn decode_user(user: &str) -> Cow<'_, [u8]> {
    if !user.contains('%') {
        return Cow::Borrowed(user.as_bytes());
    }
    Cow::Owned(percent_decode_bytes(user, |_| true))
}

/// Validate that a string contains only valid percent-encoded or allowed characters.
/// Returns `Err` with the position of the first invalid character.
pub(crate) fn validate_pct_encoded(input: &str, allowed: fn(u8) -> bool) -> Result<(), usize> {
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if escape_at(bytes, i).is_some() {
                i += 3;
                continue;
            }
            return Err(i);
        }
        if !allowed(bytes[i]) {
            return Err(i);
        }
        i += 1;
    }

    Ok(())
}

/// Octets a user part writes literally in canonical form. `;` stays escaped,
/// since a literal one starts user-params.
pub(crate) fn is_user_literal(c: u8) -> bool {
    is_user_char(c) && c != b';'
}

/// The user-literal set plus `#`, which phones and dialplans use unescaped.
fn is_user_literal_or_hash(c: u8) -> bool {
    is_user_literal(c) || c == b'#'
}

/// Octets a user-param name writes literally, where `=` would start the value.
fn is_user_param_name_literal(c: u8) -> bool {
    is_user_literal(c) && c != b'='
}

/// Octets a URI param writes literally: `paramchar` as RFC 3261 §25 writes it.
fn is_param_literal(c: u8) -> bool {
    is_param_strict(c)
}

/// RFC 3986 §3.3: `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`
fn is_pchar(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
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

/// RFC 3986 §3.5 `fragment` characters, plus `#`.
fn is_fragment_literal(c: u8) -> bool {
    is_pchar(c) || matches!(c, b'/' | b'?' | b'#')
}

/// A SIP fragment holds `@` escaped for the reason a param does.
fn is_sip_fragment_literal(c: u8) -> bool {
    is_fragment_literal(c) && c != b'@'
}

/// RFC 8141: `NSS = pchar *(pchar / "/")`.
fn is_nss_literal(c: u8) -> bool {
    is_pchar(c) || c == b'/'
}

/// RFC 8141: r-, q- and f-components are `*( pchar / "/" / "?" )`.
fn is_rqf_literal(c: u8) -> bool {
    is_pchar(c) || matches!(c, b'/' | b'?')
}

/// RFC 8141: `ldh = alphanum / "-"`.
fn is_ldh(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-'
}

fn never(_: u8) -> bool {
    false
}

/// Canonize one component: an escape of an octet in `decode` and a literal
/// octet in `keep` are written literally, every other octet as uppercase `%XX`.
///
/// `decode` must be a subset of `keep`, which then makes the result
/// idempotent. A `%` not starting a valid escape is the octet `%`.
fn canonize(input: &str, decode: fn(u8) -> bool, keep: fn(u8) -> bool) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match escape_at(bytes, i) {
            Some(b) => {
                push_octet(&mut out, b, decode);
                i += 3;
            }
            None => {
                push_octet(&mut out, bytes[i], keep);
                i += 1;
            }
        }
    }
    out
}

fn push_octet(out: &mut String, b: u8, literal: fn(u8) -> bool) {
    if b.is_ascii() && literal(b) {
        out.push(b as char);
    } else {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        out.push('%');
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0F) as usize] as char);
    }
}

/// Lowercase every literal octet, leaving escape hex uppercase.
fn lowercase_literals(s: String) -> String {
    let mut out = s.into_bytes();
    let mut i = 0;
    while i < out.len() {
        if out[i] == b'%' {
            i += 3;
        } else {
            out[i].make_ascii_lowercase();
            i += 1;
        }
    }
    // SAFETY: canonized text is ASCII, and lowercasing keeps it ASCII.
    unsafe { String::from_utf8_unchecked(out) }
}

/// Canonize a SIP user part or user-param value.
pub(crate) fn canonize_user(input: &str) -> String {
    canonize(input, is_unreserved, is_user_literal_or_hash)
}

/// Canonize a user-param name.
pub(crate) fn canonize_user_param_name(input: &str) -> String {
    canonize(input, is_unreserved, is_user_param_name_literal)
}

/// Canonize a password.
pub(crate) fn canonize_password(input: &str) -> String {
    canonize(input, is_unreserved, is_password_char)
}

/// Canonize a SIP or tel: param name or value.
pub(crate) fn canonize_param(input: &str) -> String {
    canonize(input, is_unreserved, is_param_literal)
}

/// Canonize a SIP URI header name or value.
pub(crate) fn canonize_header(input: &str) -> String {
    canonize(input, is_unreserved, is_hnv_char)
}

/// Canonize a tel: number.
pub(crate) fn canonize_tel_number(input: &str) -> String {
    canonize(input, never, is_user_literal_or_hash)
}

/// Canonize a SIP fragment.
pub(crate) fn canonize_sip_fragment(input: &str) -> String {
    canonize(input, never, is_sip_fragment_literal)
}

/// Canonize a tel: fragment or URN f-component.
pub(crate) fn canonize_fragment(input: &str) -> String {
    canonize(input, never, is_fragment_literal)
}

/// Canonize a URN NSS.
pub(crate) fn canonize_nss(input: &str) -> String {
    canonize(input, never, is_nss_literal)
}

/// Canonize a URN r-component. A `?` before `=` is escaped, since `?=` would
/// start the q-component.
pub(crate) fn canonize_urn_r(input: &str) -> String {
    canonize(input, never, is_rqf_literal).replace("?=", "%3F=")
}

/// Canonize a URN q-component.
pub(crate) fn canonize_urn_q(input: &str) -> String {
    canonize(input, never, is_rqf_literal)
}

/// Canonize a URN NID, lowercase.
pub(crate) fn canonize_nid(input: &str) -> String {
    lowercase_literals(canonize(input, never, is_ldh))
}

/// Canonize a hostname, lowercase with uppercase escape hex.
pub(crate) fn canonize_hostname(input: &str) -> String {
    lowercase_literals(canonize(input, is_unreserved, is_unreserved))
}

/// Percent-encode a URI-header name or value into the canonical form
/// returned by [`crate::SipUri::header`].
///
/// Bytes in the RFC 3261 §25 `hnv-unreserved` + `unreserved` set stay
/// literal; every other byte — including each byte of a multi-byte UTF-8
/// character — is emitted as `%XX` with uppercase hex. `hname` and `hvalue`
/// share the character set, so one function covers both.
///
/// The input is the *decoded* logical value: `%` is data and becomes `%25`.
/// Feeding an already-encoded string double-encodes it.
///
/// Returns [`Cow::Borrowed`] when no byte needs encoding.
pub fn encode_uri_header(s: &str) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    let Some(first) = bytes
        .iter()
        .position(|&b| !is_hnv_char(b))
    else {
        return Cow::Borrowed(s);
    };

    let mut out = String::with_capacity(bytes.len() + 2 * (bytes.len() - first));
    out.push_str(&s[..first]);
    for &b in &bytes[first..] {
        push_octet(&mut out, b, is_hnv_char);
    }
    Cow::Owned(out)
}

/// How an input starts, before any type-specific parsing.
pub(crate) enum SchemeSplit<'a> {
    /// A scheme and the text after its `:`.
    Named(&'a str, &'a str),
    /// A `:` whose prefix is not RFC 3986 scheme syntax, e.g. `<sip:…>`.
    Invalid,
    /// No `:`, or `host:port` with the scheme left out.
    Absent,
}

/// Schemes this crate parses; always read as schemes, even before digits.
const KNOWN_SCHEMES: [&str; 4] = ["sip", "sips", "tel", "urn"];

pub(crate) fn split_scheme(input: &str) -> SchemeSplit<'_> {
    let Some((scheme, rest)) = input.split_once(':') else {
        return SchemeSplit::Absent;
    };
    if KNOWN_SCHEMES
        .iter()
        .any(|k| scheme.eq_ignore_ascii_case(k))
    {
        return SchemeSplit::Named(scheme, rest);
    }
    if !is_rfc3986_scheme(scheme) {
        // `user@host:port` and `[v6]:port` reach their first `:` inside the authority.
        if scheme.contains('@') || scheme.starts_with('[') {
            return SchemeSplit::Absent;
        }
        return SchemeSplit::Invalid;
    }
    // `example.com:5060` is a host and port with no scheme, not scheme `example.com`.
    let port = rest
        .split([';', '?', '#', '/', '>'])
        .next()
        .unwrap_or_default();
    if !port.is_empty()
        && port
            .bytes()
            .all(|b| b.is_ascii_digit())
    {
        return SchemeSplit::Absent;
    }
    SchemeSplit::Named(scheme, rest)
}

/// RFC 3986 §3.1: `scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`
pub(crate) fn is_rfc3986_scheme(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

/// Find the `@` delimiter that separates userinfo from hostport in a SIP URI.
///
/// Uses the sofia-sip two-phase algorithm (url.c:616-626): scan to the first
/// `@/;?#`, then take the first `@` from there on. Everything before it is
/// userinfo. Without userinfo, a literal `@` in a param or header value is
/// therefore taken as the delimiter; see docs/design-rationale.md.
pub(crate) fn find_userinfo_at(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;

    // Phase 1: scan to first "@/;?#"
    while i < bytes.len() && !matches!(bytes[i], b'@' | b'/' | b';' | b'?' | b'#') {
        i += 1;
    }

    // Phase 2: from that point, scan for '@'
    while i < bytes.len() {
        if bytes[i] == b'@' {
            return Some(i);
        }
        i += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreserved_chars() {
        for c in b"abcABC019-_.!~*'()" {
            assert!(is_unreserved(*c), "expected unreserved: {:?}", *c as char);
        }
        for c in b"@:;/?#[]&=+$,\" " {
            assert!(!is_unreserved(*c), "expected reserved: {:?}", *c as char);
        }
    }

    #[test]
    fn percent_decode_unreserved() {
        assert_eq!(percent_decode("%2E", is_unreserved), ".");
        assert_eq!(percent_decode("%41", is_unreserved), "A");
        assert_eq!(percent_decode("%20", is_unreserved), "%20");
    }

    #[test]
    fn percent_decode_normalizes_hex_case() {
        assert_eq!(percent_decode("%3d", is_unreserved), "%3D");
        assert_eq!(percent_decode("%2f", is_unreserved), "%2F");
    }

    #[test]
    fn find_at_basic() {
        assert_eq!(find_userinfo_at("user@host"), Some(4));
        assert_eq!(find_userinfo_at("host"), None);
        assert_eq!(find_userinfo_at("u@h?From=foo@bar"), Some(1));
    }

    #[test]
    fn encode_uri_header_borrows_when_clean() {
        assert!(matches!(encode_uri_header(""), Cow::Borrowed("")));
        assert!(matches!(
            encode_uri_header("a-zA-Z0-9[]/?:+$"),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn encode_uri_header_escapes_non_hnv() {
        assert_eq!(
            encode_uri_header("12345@example.com;to-tag=abc"),
            "12345%40example.com%3Bto-tag%3Dabc"
        );
        assert_eq!(encode_uri_header("%"), "%25");
        assert_eq!(encode_uri_header("a b"), "a%20b");
        assert_eq!(encode_uri_header("é"), "%C3%A9");
    }

    #[test]
    fn encode_uri_header_matches_canonical_form() {
        let encoded = encode_uri_header("12345@example.com;to-tag=abc");
        assert_eq!(canonize_header(&encoded), encoded.as_ref());
    }

    #[test]
    fn decode_user_borrows_when_clean() {
        assert!(matches!(decode_user(""), Cow::Borrowed(b"")));
        assert!(matches!(
            decode_user("+15551234567;cpc=emergency"),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn decode_user_decodes_every_escape() {
        assert_eq!(decode_user("%2B1555").as_ref(), b"+1555");
        assert_eq!(decode_user("%22foo%22").as_ref(), b"\"foo\"");
        assert_eq!(decode_user("a%3Bb").as_ref(), b"a;b");
        assert_eq!(decode_user("%2b%3b").as_ref(), b"+;");
        assert_eq!(decode_user("%FF").as_ref(), b"\xFF");
    }

    #[test]
    fn decode_user_keeps_malformed_percent() {
        assert_eq!(decode_user("100%").as_ref(), b"100%");
        assert_eq!(decode_user("a%2").as_ref(), b"a%2");
        assert_eq!(decode_user("%zz").as_ref(), b"%zz");
    }

    #[test]
    fn decode_user_matches_decoding_canonical_form() {
        let raw = "%2b%22foo%22%3bcpc%3Demergency";
        assert_eq!(decode_user(raw), decode_user(&canonize_user(raw)));
    }

    #[test]
    fn canonizers_are_idempotent() {
        let fns: [fn(&str) -> String; 13] = [
            canonize_user,
            canonize_user_param_name,
            canonize_password,
            canonize_param,
            canonize_header,
            canonize_tel_number,
            canonize_sip_fragment,
            canonize_fragment,
            canonize_nss,
            canonize_urn_r,
            canonize_urn_q,
            canonize_nid,
            canonize_hostname,
        ];
        let input = "aZ%41%3b;=?@#% %zz%e2%9c%93é/:&+$,[]!~*'()";
        for f in fns {
            let once = f(input);
            assert!(once.is_ascii(), "{once}");
            assert_eq!(f(&once), once);
        }
    }

    #[test]
    fn canonize_sip_user() {
        // %2E (.) should be decoded, %40 (@) should stay encoded
        assert_eq!(canonize_user("pekka%2Epessi"), "pekka.pessi");
        assert_eq!(canonize_user("%22foo%22"), "%22foo%22");
    }
}
