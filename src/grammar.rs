//! RFC grammar classes deciding warnings, and the splits the parser makes.

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
fn is_user_unreserved(c: u8) -> bool {
    matches!(c, b'&' | b'=' | b'+' | b'$' | b',' | b';' | b'?' | b'/')
}

/// RFC 3261 §25: `param-unreserved = "[" / "]" / "/" / ":" / "&" / "+" / "$"`
fn is_param_unreserved(c: u8) -> bool {
    matches!(c, b'[' | b']' | b'/' | b':' | b'&' | b'+' | b'$')
}

/// RFC 3261 §25: `hnv-unreserved = "[" / "]" / "/" / "?" / ":" / "+" / "$"`
fn is_hnv_unreserved(c: u8) -> bool {
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

/// `paramchar` as RFC 3261 §25 writes it: unreserved + param-unreserved.
pub(crate) fn is_param_strict(c: u8) -> bool {
    is_unreserved(c) || is_param_unreserved(c)
}

/// RFC 3966: `pname = 1*( alphanum / "-" )`
pub(crate) fn is_tel_pname_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-'
}

/// RFC 3966: `paramchar = param-unreserved / unreserved / pct-encoded`, whose
/// `param-unreserved` matches RFC 3261's.
pub(crate) fn is_tel_paramchar(c: u8) -> bool {
    is_param_strict(c)
}

/// Characters allowed unescaped in header names/values:
/// unreserved + hnv-unreserved
pub(crate) fn is_hnv_char(c: u8) -> bool {
    is_unreserved(c) || is_hnv_unreserved(c)
}

/// Validate that a string contains only valid percent-encoded or allowed characters.
/// Returns `Err` with the position of the first invalid character.
pub(crate) fn validate_pct_encoded(input: &str, allowed: fn(u8) -> bool) -> Result<(), usize> {
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 < bytes.len()
                && bytes[i + 1].is_ascii_hexdigit()
                && bytes[i + 2].is_ascii_hexdigit()
            {
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
fn is_rfc3986_scheme(s: &str) -> bool {
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
    fn find_at_basic() {
        assert_eq!(find_userinfo_at("user@host"), Some(4));
        assert_eq!(find_userinfo_at("host"), None);
        assert_eq!(find_userinfo_at("u@h?From=foo@bar"), Some(1));
    }
}
