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

/// Percent-decode a string, applying a component-specific filter.
///
/// Only decodes `%XX` sequences where the decoded byte satisfies `allow_decoded`.
/// Characters that are reserved in this component stay encoded.
/// Hex digits in percent-encoding are normalized to uppercase.
pub(crate) fn percent_decode(input: &str, allow_decoded: fn(u8) -> bool) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
                let decoded = (hi << 4) | lo;
                if allow_decoded(decoded) {
                    out.push(decoded);
                } else {
                    // Keep encoded but normalize hex to uppercase
                    out.push(b'%');
                    out.push(bytes[i + 1].to_ascii_uppercase());
                    out.push(bytes[i + 2].to_ascii_uppercase());
                }
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    // SAFETY: input was valid UTF-8 and we only decoded ASCII-range bytes
    // or preserved original UTF-8 sequences
    unsafe { String::from_utf8_unchecked(out) }
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

/// Canonize a percent-encoded user component: decode unreserved + user-unreserved,
/// uppercase remaining %XX.
pub(crate) fn canonize_user(input: &str) -> String {
    percent_decode(input, is_user_char)
}

/// Canonize a percent-encoded password component.
pub(crate) fn canonize_password(input: &str) -> String {
    percent_decode(input, is_password_char)
}

/// Canonize a percent-encoded parameter component.
pub(crate) fn canonize_param(input: &str) -> String {
    percent_decode(input, is_paramchar)
}

/// Canonize a percent-encoded header component.
pub(crate) fn canonize_header(input: &str) -> String {
    percent_decode(input, is_hnv_char)
}

/// Find the `@` delimiter that separates userinfo from hostport in a SIP URI.
///
/// Uses the sofia-sip two-phase algorithm (url.c:616-626). `@` is not in
/// user-unreserved per RFC 3261, but `/;?#` are. Phase 1 scans to the first
/// `@/;?#`. Phase 2 scans forward from there looking for `@`. If found,
/// everything before it is userinfo.
///
/// This correctly handles `@` in headers (`?From=foo@bar`) which is
/// technically non-conformant but universal in practice: Phase 1 reaches `?`
/// (a user-unreserved char), Phase 2 scans past the `@` in headers only if
/// there's a real `@` delimiter earlier.
pub(crate) fn find_userinfo_at(s: &str) -> Option<usize> {
    // Sofia-sip algorithm (url.c line 616-626):
    // 1. Find first char in "@/;?#" (easy delimiters or user-unreserved)
    // 2. From there, scan forward looking for '@'
    // 3. If found, everything before '@' is userinfo
    //
    // This works because in a SIP URI without userinfo like
    // "host:port;params?headers", the first /;?# terminates the
    // host scan, and there's no @ after it in the hostport+params
    // section (@ would be %40 there). But with userinfo, the /;?#
    // are part of the user, and the real @ follows them.
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
    fn canonize_sip_user() {
        // %2E (.) should be decoded, %40 (@) should stay encoded
        assert_eq!(canonize_user("pekka%2Epessi"), "pekka.pessi");
        assert_eq!(canonize_user("%22foo%22"), "%22foo%22");
    }
}
