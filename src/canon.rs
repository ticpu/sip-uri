//! Canonical form of each URI component, and the literal sets deciding it.

use std::borrow::Cow;

/// RFC 3261 §25: `unreserved = alphanum / mark`
fn is_unreserved(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
        )
}

/// Octets a user part writes literally: `unreserved` and `user-unreserved`
/// but `;`, since a literal one starts user-params.
fn is_user_literal(c: u8) -> bool {
    is_unreserved(c) || matches!(c, b'&' | b'=' | b'+' | b'$' | b',' | b'?' | b'/')
}

/// The user-literal set plus `#`, which phones and dialplans use unescaped.
fn is_user_literal_or_hash(c: u8) -> bool {
    is_user_literal(c) || c == b'#'
}

/// Octets a user-param name writes literally, where `=` would start the value.
fn is_user_param_name_literal(c: u8) -> bool {
    is_user_literal(c) && c != b'='
}

/// RFC 3261 §25 password characters.
fn is_password_literal(c: u8) -> bool {
    is_unreserved(c) || matches!(c, b'&' | b'=' | b'+' | b'$' | b',')
}

/// RFC 3261 §25: `unreserved / param-unreserved`.
fn is_param_literal(c: u8) -> bool {
    is_unreserved(c) || matches!(c, b'[' | b']' | b'/' | b':' | b'&' | b'+' | b'$')
}

/// RFC 3261 §25: `unreserved / hnv-unreserved`.
fn is_hnv_literal(c: u8) -> bool {
    is_unreserved(c) || matches!(c, b'[' | b']' | b'/' | b'?' | b':' | b'+' | b'$')
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

/// Schemes with their own URI type, never held by an `OtherUri`.
const KNOWN_SCHEMES: [&str; 4] = ["sip", "sips", "tel", "urn"];

pub(crate) fn is_known_scheme(s: &str) -> bool {
    KNOWN_SCHEMES
        .iter()
        .any(|k| s.eq_ignore_ascii_case(k))
}

/// RFC 3986 §3.1: `scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`
pub(crate) fn is_scheme(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

/// The octet escaped by a well-formed `%XX` starting at `i`.
fn escape_at(bytes: &[u8], i: usize) -> Option<u8> {
    if bytes.get(i) != Some(&b'%') {
        return None;
    }
    let hi = char::from(*bytes.get(i + 1)?).to_digit(16)?;
    let lo = char::from(*bytes.get(i + 2)?).to_digit(16)?;
    Some(((hi << 4) | lo) as u8)
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

/// Canonize a user-param name and value.
pub(crate) fn canonize_user_param(name: &str, value: Option<&str>) -> (String, Option<String>) {
    (
        canonize(name, is_unreserved, is_user_param_name_literal),
        value.map(canonize_user),
    )
}

/// Canonize a password.
pub(crate) fn canonize_password(input: &str) -> String {
    canonize(input, is_unreserved, is_password_literal)
}

/// Canonize a SIP or tel: param name and value.
pub(crate) fn canonize_param(name: &str, value: Option<&str>) -> (String, Option<String>) {
    let one = |s: &str| canonize(s, is_unreserved, is_param_literal);
    (one(name), value.map(one))
}

/// Canonize a SIP URI header name and value.
pub(crate) fn canonize_header(name: &str, value: &str) -> (String, String) {
    let one = |s: &str| canonize(s, is_unreserved, is_hnv_literal);
    (one(name), one(value))
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

/// Octets the text of an unrecognized URI writes literally: any ASCII but
/// controls, space and the `<` `>` `"` that delimit a URI in a header.
fn is_other_literal(c: u8) -> bool {
    !c.is_ascii_control() && !matches!(c, b' ' | b'<' | b'>' | b'"')
}

/// Canonize the text of an unrecognized URI, decoding nothing.
pub(crate) fn canonize_other(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        push_octet(&mut out, b, is_other_literal);
    }
    out
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
    let bytes = user.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match escape_at(bytes, i) {
            Some(b) => {
                out.push(b);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    Cow::Owned(out)
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
        .position(|&b| !is_hnv_literal(b))
    else {
        return Cow::Borrowed(s);
    };

    let mut out = String::with_capacity(bytes.len() + 2 * (bytes.len() - first));
    out.push_str(&s[..first]);
    for &b in &bytes[first..] {
        push_octet(&mut out, b, is_hnv_literal);
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(canonize_header("", &encoded).1, encoded.as_ref());
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
        let fns: [fn(&str) -> String; 14] = [
            canonize_user,
            |s| canonize_user_param(s, None).0,
            canonize_password,
            |s| canonize_param(s, None).0,
            |s| canonize_header(s, "").0,
            canonize_tel_number,
            canonize_sip_fragment,
            canonize_fragment,
            canonize_nss,
            canonize_urn_r,
            canonize_urn_q,
            canonize_nid,
            canonize_hostname,
            canonize_other,
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
        assert_eq!(canonize_user("pekka%2Epessi"), "pekka.pessi");
        assert_eq!(canonize_user("%22foo%22"), "%22foo%22");
        assert_eq!(canonize_user("%2e%23#"), ".%23#");
    }
}
