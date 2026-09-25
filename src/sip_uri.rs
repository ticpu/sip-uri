use std::fmt;
use std::str::FromStr;

use crate::error::ParseSipUriError;
use crate::host::Host;
use crate::params;
use crate::parse;
use crate::warning::{Component, Parsed, WarningCode, Warnings};

type Params = Vec<(String, Option<String>)>;
type Headers = Vec<(String, String)>;

type UserinfoResult = Result<(Option<String>, Params, Option<String>), ParseSipUriError>;
type HostportResult =
    Result<(Host, Option<u16>, Params, Headers, Option<String>), ParseSipUriError>;

/// SIP or SIPS URI per RFC 3261 §19.
///
/// Supports the full grammar including user-params (`;` within userinfo),
/// password, IPv6 hosts, URI parameters, and headers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SipUri {
    scheme: Scheme,
    user: Option<String>,
    user_params: Vec<(String, Option<String>)>,
    password: Option<String>,
    host: Host,
    port: Option<u16>,
    params: Vec<(String, Option<String>)>,
    headers: Vec<(String, String)>,
    fragment: Option<String>,
}

/// SIP URI scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Scheme {
    /// `sip:` (default port 5060)
    Sip,
    /// `sips:` (default port 5061)
    Sips,
}

impl Scheme {
    /// The scheme name as it appears before `:`, lowercase.
    pub fn as_str(self) -> &'static str {
        match self {
            Scheme::Sip => "sip",
            Scheme::Sips => "sips",
        }
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl SipUri {
    /// Create a new SIP URI with the given host and `sip:` scheme.
    pub fn new(host: Host) -> Self {
        SipUri {
            scheme: Scheme::Sip,
            user: None,
            user_params: Vec::new(),
            password: None,
            host,
            port: None,
            params: Vec::new(),
            headers: Vec::new(),
            fragment: None,
        }
    }

    /// Set the URI scheme.
    pub fn with_scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = scheme;
        self
    }

    /// Set the user part.
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Replace all user-params (parameters within the userinfo, before `@`).
    pub fn with_user_params(mut self, params: Vec<(String, Option<String>)>) -> Self {
        self.user_params = params;
        self
    }

    /// Add a single user-param (parameter within the userinfo, before `@`).
    pub fn with_user_param(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        self.user_params
            .push((name.into(), value));
        self
    }

    /// Set the password.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Set the port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Add a URI parameter.
    pub fn with_param(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        self.params
            .push((name.into(), value));
        self
    }

    /// Add a header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .push((name.into(), value.into()));
        self
    }

    /// The URI scheme (`sip` or `sips`).
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// The user part (without user-params or password).
    pub fn user(&self) -> Option<&str> {
        self.user
            .as_deref()
    }

    /// Parameters within the userinfo (before `@`), separated by `;` in the user part.
    ///
    /// Common in tel-style SIP URIs, e.g., `sip:+15551234567;cpc=emergency@host`.
    pub fn user_params(&self) -> &[(String, Option<String>)] {
        &self.user_params
    }

    /// The password component (deprecated by RFC 3261 but still parseable).
    pub fn password(&self) -> Option<&str> {
        self.password
            .as_deref()
    }

    /// The host component.
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// The explicit port, if specified.
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// URI parameters (after host, separated by `;`).
    pub fn params(&self) -> &[(String, Option<String>)] {
        &self.params
    }

    /// Look up a URI parameter by name (case-insensitive).
    pub fn param(&self, name: &str) -> Option<&Option<String>> {
        params::find_param(&self.params, name)
    }

    /// URI headers (after `?`).
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Look up a header by name (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The fragment component (after `#`), if present.
    ///
    /// RFC 3261 does not define fragments for SIP URIs, but sofia-sip
    /// and real-world implementations accept them permissively.
    pub fn fragment(&self) -> Option<&str> {
        self.fragment
            .as_deref()
    }

    /// Set the fragment component.
    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        self.fragment = Some(fragment.into());
        self
    }

    /// Convenience: `user@host:port` or `host:port` string.
    pub fn user_host(&self) -> String {
        let mut s = String::new();
        if let Some(ref u) = self.user {
            s.push_str(u);
            s.push('@');
        }
        s.push_str(
            &self
                .host
                .to_string(),
        );
        if let Some(p) = self.port {
            s.push(':');
            s.push_str(&p.to_string());
        }
        s
    }
}

impl FromStr for SipUri {
    type Err = ParseSipUriError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse_with_warnings(input).map(|parsed| parsed.value)
    }
}

impl SipUri {
    /// Parse, reporting accepted grammar breaches beside the value.
    ///
    /// Accepts exactly what [`FromStr`] accepts.
    ///
    /// ```
    /// use sip_uri::{SipUri, WarningCode};
    ///
    /// let parsed = SipUri::parse_with_warnings("sip:host:+5060").unwrap();
    /// assert_eq!(parsed.value.port(), Some(5060));
    /// assert_eq!(parsed.warnings[0].code, WarningCode::SignedPort);
    /// ```
    pub fn parse_with_warnings(input: &str) -> Result<Parsed<Self>, ParseSipUriError> {
        let mut warnings = Warnings::new(input);
        let uri = Self::parse_into(input, &mut warnings)?;
        Ok(warnings.finish(uri))
    }

    fn parse_into(input: &str, warnings: &mut Warnings) -> Result<Self, ParseSipUriError> {
        let err = |msg: &str| ParseSipUriError(msg.to_string());

        // 1. Scheme detection
        let colon_pos = input
            .find(':')
            .ok_or_else(|| err("missing scheme"))?;
        let scheme_str = &input[..colon_pos];
        let scheme = if scheme_str.eq_ignore_ascii_case("sip") {
            Scheme::Sip
        } else if scheme_str.eq_ignore_ascii_case("sips") {
            Scheme::Sips
        } else {
            return Err(err("scheme is not sip or sips"));
        };

        let rest = &input[colon_pos + 1..];

        let (userinfo, hostport_rest) = split_userinfo_host(rest)?;

        let (user, user_params, password) = if let Some(uinfo) = userinfo {
            parse_userinfo(uinfo, warnings)?
        } else {
            (None, Vec::new(), None)
        };

        let (host, port, uri_params, headers, fragment) =
            parse_hostport_params_headers(hostport_rest, warnings)?;

        Ok(SipUri {
            scheme,
            user,
            user_params,
            password,
            host,
            port,
            params: uri_params,
            headers,
            fragment,
        })
    }
}

/// Split a SIP URI (after scheme:) into optional userinfo and the rest (host onwards).
///
/// Uses the sofia-sip algorithm: scan for `@` looking past `/;?#` which are
/// allowed unescaped in the SIP user part.
fn split_userinfo_host(s: &str) -> Result<(Option<&str>, &str), ParseSipUriError> {
    let err = |msg: &str| ParseSipUriError(msg.to_string());

    if let Some(at_pos) = parse::find_userinfo_at(s) {
        if at_pos == 0 {
            return Err(err("empty userinfo before @"));
        }
        let userinfo = &s[..at_pos];
        let rest = &s[at_pos + 1..];
        if rest.is_empty() {
            return Err(err("missing host after @"));
        }
        Ok((Some(userinfo), rest))
    } else {
        // No @, the whole thing is hostport+params+headers
        Ok((None, s))
    }
}

/// Parse the userinfo portion into (user, user_params, password).
///
/// Userinfo structure: `user [*(";" user-param)] [":" password]`
fn parse_userinfo(s: &str, warnings: &mut Warnings) -> UserinfoResult {
    let err = |msg: &str| ParseSipUriError(msg.to_string());

    // `:` is not user-unreserved, so the first one starts the password.
    let (user_and_params, password) = if let Some(colon_pos) = s.find(':') {
        let pwd = &s[colon_pos + 1..];
        warnings.charset(Component::Password, pwd, parse::is_password_char);
        (&s[..colon_pos], Some(parse::canonize_password(pwd)))
    } else {
        (s, None)
    };

    let (user_part, params_str) = match user_and_params.split_once(';') {
        Some((user, params)) => (user, Some(params)),
        None => (user_and_params, None),
    };

    if user_part.is_empty() {
        if params_str.is_some() {
            return Err(err("empty user before ';'"));
        }
        warnings.push(Component::User, WarningCode::PasswordWithoutUser, s, 0);
        return Ok((None, Vec::new(), password));
    }

    warnings.charset(Component::User, user_part, parse::is_user_char);
    if let Some(q) = user_and_params.find('?') {
        warnings.push(
            Component::User,
            WarningCode::HeaderShapedUser,
            user_and_params,
            q,
        );
    }

    let user_params = match params_str {
        Some(p) => params::parse_params(p, &params::USER_PARAMS, warnings)
            .map_err(|e| err(&format!("user param: {e}")))?,
        None => Vec::new(),
    };

    Ok((Some(parse::canonize_user(user_part)), user_params, password))
}

/// Parse host, optional port, URI params, and headers from the portion after `@` (or after scheme: if no userinfo).
fn parse_hostport_params_headers(s: &str, warnings: &mut Warnings) -> HostportResult {
    let err = |msg: &str| ParseSipUriError(msg.to_string());

    let (host, consumed) = Host::parse_from_uri(s, warnings).map_err(|e| err(&e))?;

    let rest = &s[consumed..];

    let (port, rest) = if let Some(after) = rest.strip_prefix(':') {
        let end = after
            .find([';', '?', '#', '>'])
            .unwrap_or(after.len());
        let port_str = &after[..end];

        if port_str.is_empty() {
            warnings.push(Component::Port, WarningCode::EmptyPort, rest, 0);
            (None, &after[end..])
        } else {
            if port_str.starts_with('+') {
                warnings.push(Component::Port, WarningCode::SignedPort, port_str, 0);
            }
            let port: u16 = port_str
                .parse()
                .map_err(|_| err("port is not a number in 0-65535"))?;
            (Some(port), &after[end..])
        }
    } else {
        (None, rest)
    };

    let (rest, fragment) = if let Some(hash_pos) = rest.find('#') {
        let frag = &rest[hash_pos + 1..];
        let frag = if frag.is_empty() {
            warnings.push(
                Component::Fragment,
                WarningCode::EmptyFragment,
                rest,
                hash_pos,
            );
            None
        } else {
            warnings.push(
                Component::Fragment,
                WarningCode::UnexpectedFragment,
                rest,
                hash_pos,
            );
            Some(frag.to_string())
        };
        (&rest[..hash_pos], frag)
    } else {
        (rest, None)
    };

    let (params_str, headers_str) = if let Some(rest) = rest.strip_prefix(';') {
        match rest.split_once('?') {
            Some((params, headers)) => (Some(params), Some(headers)),
            None => (Some(rest), None),
        }
    } else if let Some(rest) = rest.strip_prefix('?') {
        (None, Some(rest))
    } else if rest.is_empty() {
        (None, None)
    } else {
        return Err(err("unexpected character after host/port"));
    };

    let uri_params = match params_str {
        Some(p) => params::parse_params(p, &params::SIP_PARAMS, warnings)
            .map_err(|e| err(&format!("URI param: {e}")))?,
        None => Vec::new(),
    };

    let headers = match headers_str {
        Some(h) => params::parse_headers(h, warnings).map_err(|e| err(&format!("header: {e}")))?,
        None => Vec::new(),
    };

    Ok((host, port, uri_params, headers, fragment))
}

impl fmt::Display for SipUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.scheme)?;

        // Userinfo
        if let Some(ref user) = self.user {
            write!(f, "{user}")?;

            // User-params
            params::format_params(&self.user_params, f)?;

            // Password
            if let Some(ref pwd) = self.password {
                write!(f, ":{pwd}")?;
            }

            write!(f, "@")?;
        } else if let Some(ref pwd) = self.password {
            write!(f, ":{pwd}@")?;
        }

        write!(f, "{}", self.host)?;

        // Port
        if let Some(port) = self.port {
            write!(f, ":{port}")?;
        }

        // URI parameters
        params::format_params(&self.params, f)?;

        // Headers
        params::format_headers(&self.headers, f)?;

        // Fragment
        if let Some(ref frag) = self.fragment {
            write!(f, "#{frag}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn parse_simple() {
        let uri: SipUri = "sip:joe@example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.scheme(), Scheme::Sip);
        assert_eq!(uri.user(), Some("joe"));
        assert_eq!(uri.host(), &Host::Hostname("example.com".into()));
        assert_eq!(uri.port(), None);
    }

    #[test]
    fn parse_minimal_user_host() {
        let uri: SipUri = "sip:u@h"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), Some("u"));
        assert_eq!(uri.host(), &Host::Hostname("h".into()));
    }

    #[test]
    fn parse_host_only() {
        let uri: SipUri = "sip:test.host"
            .parse()
            .unwrap();
        assert_eq!(uri.user(), None);
        assert_eq!(uri.host(), &Host::Hostname("test.host".into()));
    }

    #[test]
    fn parse_ipv4_host() {
        let uri: SipUri = "sip:172.21.55.55"
            .parse()
            .unwrap();
        assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)));
    }

    #[test]
    fn parse_ipv4_with_port() {
        let uri: SipUri = "sip:172.21.55.55:5060"
            .parse()
            .unwrap();
        assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)));
        assert_eq!(uri.port(), Some(5060));
    }

    #[test]
    fn parse_full_sips() {
        let uri: SipUri = "sips:user:pass@host:32;param=1?From=foo@bar&To=bar@baz"
            .parse()
            .unwrap();
        assert_eq!(uri.scheme(), Scheme::Sips);
        assert_eq!(uri.user(), Some("user"));
        assert_eq!(uri.password(), Some("pass"));
        assert_eq!(uri.host(), &Host::Hostname("host".into()));
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
        assert_eq!(uri.scheme(), Scheme::Sip);
        assert_eq!(uri.user(), Some("test"));
        assert_eq!(uri.port(), Some(55));
    }

    #[test]
    fn parse_empty_port() {
        let uri: SipUri = "SIP:test@127.0.0.1:"
            .parse()
            .unwrap();
        assert_eq!(uri.scheme(), Scheme::Sip);
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
            uri.host(),
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
        assert_eq!(uri.param("transport"), Some(&Some("tcp".into())));
        assert_eq!(uri.param("maddr"), Some(&Some("127.0.0.1".into())));
    }

    #[test]
    fn parse_params_without_value() {
        let uri: SipUri = "sip:u:p@host:5060;user=phone;ttl=1;isfocus"
            .parse()
            .unwrap();
        assert_eq!(uri.param("user"), Some(&Some("phone".into())));
        assert_eq!(uri.param("isfocus"), Some(&None));
    }

    #[test]
    fn invalid_double_colon_port() {
        assert!("sip:test@127.0.0.1::55"
            .parse::<SipUri>()
            .is_err());
    }

    #[test]
    fn invalid_trailing_colon_port() {
        assert!("sip:test@127.0.0.1:55:"
            .parse::<SipUri>()
            .is_err());
    }

    #[test]
    fn invalid_non_numeric_port() {
        assert!("sip:test@127.0.0.1:sip"
            .parse::<SipUri>()
            .is_err());
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
    fn builder() {
        let uri = SipUri::new(Host::Hostname("example.com".into()))
            .with_user("alice")
            .with_param("transport", Some("tcp".into()));
        assert_eq!(uri.to_string(), "sip:alice@example.com;transport=tcp");
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
        assert_eq!(uri.param("transport"), Some(&Some("udp".into())));
    }
}
