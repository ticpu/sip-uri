use std::str::FromStr;

use super::host::parse_from_uri;
use super::params::{self, parse_headers, parse_params};
use crate::error::ParseError;
use crate::grammar::{self, SchemeSplit};
use crate::sip_uri::{Scheme, SipUri, SipUriParts};
use crate::warning::{Component, Parsed, WarningCode, Warnings};

impl FromStr for SipUri {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse_with_warnings(input).map(|parsed| parsed.value)
    }
}

impl SipUri {
    /// Parse, rejecting any grammar breach as [`ParseError::NonConformant`].
    pub fn parse_strict(input: &str) -> Result<Self, ParseError> {
        Self::parse_with_warnings(input)?.into_strict()
    }

    /// Parse, reporting accepted grammar breaches beside the value.
    ///
    /// Accepts exactly what [`FromStr`] accepts: everything except empty
    /// input and a scheme other than `sip`/`sips`.
    ///
    /// ```
    /// use sip_uri::{SipUri, WarningCode};
    ///
    /// let parsed = SipUri::parse_with_warnings("sip:host:+5060").unwrap();
    /// assert_eq!(parsed.value.port(), Some(5060));
    /// assert_eq!(parsed.warnings[0].code, WarningCode::SignedPort);
    /// ```
    pub fn parse_with_warnings(input: &str) -> Result<Parsed<Self>, ParseError> {
        parse(input)
    }
}

pub(crate) fn parse(input: &str) -> Result<Parsed<SipUri>, ParseError> {
    if input.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut warnings = Warnings::new(input);
    let mut parts = SipUriParts::default();

    let rest = match grammar::split_scheme(input) {
        SchemeSplit::Named(s, rest) if s.eq_ignore_ascii_case("sip") => {
            parts.scheme = Some(Scheme::Sip);
            rest
        }
        SchemeSplit::Named(s, rest) if s.eq_ignore_ascii_case("sips") => {
            parts.scheme = Some(Scheme::Sips);
            rest
        }
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

    let (userinfo, hostport_rest) = split_userinfo_host(rest, &mut warnings);
    if let Some(uinfo) = userinfo {
        split_userinfo(uinfo, &mut parts, &mut warnings);
    }
    split_hostport_params_headers(hostport_rest, &mut parts, &mut warnings);

    Ok(warnings.finish(SipUri::from(parts)))
}

/// Split a SIP URI (after scheme:) into optional userinfo and the rest (host onwards).
///
/// Uses the sofia-sip algorithm: scan for `@` looking past `/;?#` which are
/// allowed unescaped in the SIP user part.
fn split_userinfo_host<'a>(s: &'a str, warnings: &mut Warnings) -> (Option<&'a str>, &'a str) {
    match grammar::find_userinfo_at(s) {
        Some(0) => {
            warnings.push(Component::User, WarningCode::EmptyUserinfo, s, 0);
            (None, &s[1..])
        }
        Some(at_pos) => (Some(&s[..at_pos]), &s[at_pos + 1..]),
        None => (None, s),
    }
}

/// Split the userinfo: `user [*(";" user-param)] [":" password]`.
fn split_userinfo(s: &str, parts: &mut SipUriParts, warnings: &mut Warnings) {
    // `:` is not user-unreserved, so the first one starts the password.
    let user_and_params = if let Some(colon_pos) = s.find(':') {
        let pwd = &s[colon_pos + 1..];
        warnings.charset(Component::Password, pwd, grammar::is_password_char);
        parts.password = Some(pwd.to_string());
        &s[..colon_pos]
    } else {
        s
    };

    let (user_part, params_str) = match user_and_params.split_once(';') {
        Some((user, params)) => (user, Some(params)),
        None => (user_and_params, None),
    };

    if user_part.is_empty() {
        if params_str.is_none() {
            warnings.push(Component::User, WarningCode::PasswordWithoutUser, s, 0);
            return;
        }
        warnings.push(Component::User, WarningCode::EmptyUser, s, 0);
    }

    warnings.charset(Component::User, user_part, grammar::is_user_char);
    if let Some(q) = user_and_params.find('?') {
        warnings.push(
            Component::User,
            WarningCode::HeaderShapedUser,
            user_and_params,
            q,
        );
    }

    if let Some(p) = params_str {
        parts.user_params = parse_params(p, &params::USER_PARAMS, warnings);
    }
    parts.user = Some(user_part.to_string());
}

/// Split host, port, URI params, headers and fragment from the portion after
/// `@`, or after the scheme when there is no userinfo.
fn split_hostport_params_headers(s: &str, parts: &mut SipUriParts, warnings: &mut Warnings) {
    let (host, consumed) = parse_from_uri(s, warnings);
    parts.host = host;

    let rest = &s[consumed..];

    let rest = if let Some(after) = rest.strip_prefix(':') {
        let end = after
            .find([';', '?', '#', '>'])
            .unwrap_or(after.len());
        let port_str = &after[..end];

        if port_str.is_empty() {
            warnings.push(Component::Port, WarningCode::EmptyPort, rest, 0);
        } else {
            if port_str.starts_with('+') {
                warnings.push(Component::Port, WarningCode::SignedPort, port_str, 0);
            }
            parts.port = port_str
                .parse::<u16>()
                .ok();
            if parts
                .port
                .is_none()
            {
                warnings.push(Component::Port, WarningCode::InvalidPort, port_str, 0);
            }
        }
        &after[end..]
    } else {
        rest
    };

    let rest = if let Some(hash_pos) = rest.find('#') {
        let frag = &rest[hash_pos + 1..];
        if frag.is_empty() {
            warnings.push(
                Component::Fragment,
                WarningCode::EmptyFragment,
                rest,
                hash_pos,
            );
        } else {
            warnings.push(
                Component::Fragment,
                WarningCode::UnexpectedFragment,
                rest,
                hash_pos,
            );
            parts.fragment = Some(frag.to_string());
        }
        &rest[..hash_pos]
    } else {
        rest
    };

    let rest = match rest.find([';', '?']) {
        Some(0) => rest,
        Some(start) => {
            warnings.push(Component::Host, WarningCode::TrailingContent, rest, 0);
            &rest[start..]
        }
        None if rest.is_empty() => rest,
        None => {
            warnings.push(Component::Host, WarningCode::TrailingContent, rest, 0);
            ""
        }
    };

    let (params_str, headers_str) = if let Some(rest) = rest.strip_prefix(';') {
        match rest.split_once('?') {
            Some((params, headers)) => (Some(params), Some(headers)),
            None => (Some(rest), None),
        }
    } else if let Some(rest) = rest.strip_prefix('?') {
        (None, Some(rest))
    } else {
        (None, None)
    };

    if let Some(p) = params_str {
        parts.params = parse_params(p, &params::SIP_PARAMS, warnings);
    }
    if let Some(h) = headers_str {
        parts.headers = parse_headers(h, warnings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Host;
    use std::net::Ipv4Addr;

    #[test]
    fn parse_simple() {
        let uri: SipUri = "sip:joe@example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.scheme(), Some(Scheme::Sip));
        assert_eq!(uri.user(), Some("joe"));
        assert_eq!(uri.host(), Some(&Host::Hostname("example.com".into())));
        assert_eq!(uri.port(), None);
    }

    #[test]
    fn parse_minimal_user_host() {
        let uri: SipUri = "sip:u@h"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), Some("u"));
        assert_eq!(uri.host(), Some(&Host::Hostname("h".into())));
    }

    #[test]
    fn parse_host_only() {
        let uri: SipUri = "sip:test.host"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), None);
        assert_eq!(uri.host(), Some(&Host::Hostname("test.host".into())));
    }

    #[test]
    fn parse_ipv4_host() {
        let uri: SipUri = "sip:172.21.55.55"
            .parse()
            .unwrap();
        assert_eq!(
            uri.host(),
            Some(&Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)))
        );
    }

    #[test]
    fn parse_ipv4_with_port() {
        let uri: SipUri = "sip:172.21.55.55:5060"
            .parse()
            .unwrap();
        assert_eq!(
            uri.host(),
            Some(&Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)))
        );
        assert_eq!(uri.port(), Some(5060));
    }

    #[test]
    fn parse_full_sips() {
        let uri: SipUri = "sips:user:pass@host:32;param=1?From=foo@bar&To=bar@baz"
            .parse()
            .unwrap();
        assert_eq!(uri.scheme(), Some(Scheme::Sips));
        assert_eq!(uri.user(), Some("user"));
        assert_eq!(uri.password(), Some("pass"));
        assert_eq!(uri.host(), Some(&Host::Hostname("host".into())));
        assert_eq!(uri.port(), Some(32));
        assert_eq!(uri.params(), &[("param".into(), Some("1".into()))]);
        assert_eq!(uri.header("From"), Some("foo%40bar"));
        assert_eq!(uri.header("To"), Some("bar%40baz"));
    }

    #[test]
    fn parse_case_insensitive_scheme() {
        let uri: SipUri = "SIP:test@127.0.0.1:55"
            .parse()
            .unwrap();
        assert_eq!(uri.scheme(), Some(Scheme::Sip));
        assert_eq!(uri.user(), Some("test"));
        assert_eq!(uri.port(), Some(55));
    }

    #[test]
    fn parse_empty_port() {
        let uri: SipUri = "SIP:test@127.0.0.1:"
            .parse()
            .unwrap();
        assert_eq!(uri.scheme(), Some(Scheme::Sip));
        assert_eq!(uri.port(), None);
    }

    #[test]
    fn parse_percent_encoded_user() {
        let uri: SipUri = "sip:%22foo%22@172.21.55.55:5060"
            .parse()
            .unwrap();
        // %22 is double-quote, not unreserved, stays encoded
        assert_eq!(uri.user(), Some("%22foo%22"));
    }

    #[test]
    fn parse_user_with_slash_semicolon() {
        let uri: SipUri = "sip:user/path;tel-param:pass@host:32;param=1%3d%3d1"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), Some("user/path"));
        assert_eq!(uri.user_params(), &[("tel-param".into(), None)]);
        assert_eq!(uri.password(), Some("pass"));
        // %3d normalized to uppercase %3D
        assert_eq!(uri.params(), &[("param".into(), Some("1%3D%3D1".into()))]);
    }

    #[test]
    fn parse_reserved_chars_in_user_ipv6() {
        let uri: SipUri = "sip:&=+$,;?/:&=+$,@[::1]:56001;param=+$,/:@&"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), Some("&=+$,"));
        // `;` splits user from user-params, `?/` is a param name (no `=`),
        // and `:` splits the remaining `&=+$,` as the password
        assert_eq!(uri.user_params(), &[("?/".into(), None)]);
        assert_eq!(uri.password(), Some("&=+$,"));
        assert_eq!(
            uri.host()
                .unwrap(),
            &Host::IPv6(
                "::1"
                    .parse()
                    .unwrap()
            )
        );
        assert_eq!(uri.port(), Some(56001));
    }

    #[test]
    fn parse_hash_in_user() {
        // Sofia-sip compatibility: phones put unescaped # in user
        let uri: SipUri = "SIP:#**00**#;foo=/bar@127.0.0.1"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), Some("#**00**#"));
        assert_eq!(uri.user_params(), &[("foo".into(), Some("/bar".into()))]);
    }

    #[test]
    fn parse_transport_params() {
        let uri: SipUri = "sip:u:p@host:5060;maddr=127.0.0.1;transport=tcp"
            .parse()
            .unwrap();
        assert_eq!(uri.param("transport"), Some(Some("tcp")));
        assert_eq!(uri.param("maddr"), Some(Some("127.0.0.1")));
    }

    #[test]
    fn parse_params_without_value() {
        let uri: SipUri = "sip:u:p@host:5060;user=phone;ttl=1;isfocus"
            .parse()
            .unwrap();
        assert_eq!(uri.param("user"), Some(Some("phone")));
        assert_eq!(uri.param("isfocus"), Some(None));
    }

    #[test]
    fn invalid_ports_are_dropped() {
        for input in [
            "sip:test@127.0.0.1::55",
            "sip:test@127.0.0.1:55:",
            "sip:test@127.0.0.1:sip",
        ] {
            let parsed = SipUri::parse_with_warnings(input).unwrap();
            assert_eq!(
                parsed
                    .value
                    .port(),
                None,
                "{input}"
            );
            assert!(
                parsed
                    .warnings
                    .iter()
                    .any(|w| w.code == WarningCode::InvalidPort),
                "{input}: {:?}",
                parsed.warnings
            );
        }
    }

    #[test]
    fn display_roundtrip_simple() {
        let input = "sip:joe@example.com";
        let uri: SipUri = input
            .parse()
            .unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn display_roundtrip_full() {
        let uri: SipUri = "sips:user:pass@host:32;param=1?From=foo@bar&To=bar@baz"
            .parse()
            .unwrap();
        assert_eq!(
            uri.to_string(),
            "sips:user:pass@host:32;param=1?From=foo%40bar&To=bar%40baz"
        );
    }

    #[test]
    fn display_keeps_password_without_user() {
        let uri: SipUri = "sip::pass@host"
            .parse()
            .unwrap();
        assert_eq!(uri.password(), Some("pass"));
        assert_eq!(uri.to_string(), "sip::pass@host");
    }

    #[test]
    fn user_host_convenience() {
        let uri: SipUri = "sip:alice@example.com:5060"
            .parse()
            .unwrap();
        assert_eq!(uri.user_host(), "alice@example.com:5060");
    }

    #[test]
    fn no_user_with_host_params() {
        let uri: SipUri = "sip:172.21.55.55:5060;transport=udp"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), None);
        assert_eq!(uri.port(), Some(5060));
        assert_eq!(uri.param("transport"), Some(Some("udp")));
    }
}
