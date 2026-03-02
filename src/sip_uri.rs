use std::fmt;
use std::str::FromStr;

use crate::error::ParseSipUriError;
use crate::host::Host;
use crate::params;
use crate::parse;

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

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scheme::Sip => write!(f, "sip"),
            Scheme::Sips => write!(f, "sips"),
        }
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
            return Err(err(&format!("unknown scheme '{scheme_str}'")));
        };

        let rest = &input[colon_pos + 1..];

        // 2. Split userinfo from hostport+params+headers
        // SIP user-part allows ;/?/ unescaped, so we use the sofia-sip approach:
        // find @ by scanning past those chars
        let (userinfo, hostport_rest) = split_userinfo_host(rest)?;

        // 3. Parse userinfo if present
        let (user, user_params, password) = if let Some(uinfo) = userinfo {
            parse_userinfo(uinfo)?
        } else {
            (None, Vec::new(), None)
        };

        // 4. Parse host, port, params, headers, fragment from the rest
        let (host, port, uri_params, headers, fragment) =
            parse_hostport_params_headers(hostport_rest)?;

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

    // Find the `@` delimiter. In SIP, the user part can contain ;/?/ and even #
    // (non-conformant phones), so we can't just scan for the first special char.
    // Strategy: find the last `@` before any unescaped `?` that starts headers
    // (headers can contain `@` too, e.g., From=foo@bar).
    //
    // Actually, per the ABNF, `@` is not in user-unreserved, so any literal `@`
    // in the pre-headers portion is THE delimiter. We find the rightmost one
    // before `?` to handle edge cases.
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
///
/// The tricky part: `;` and `:` are both allowed in the user part as
/// user-unreserved chars. But the RFC grammar says user-params are
/// separated by `;` and password follows `:`.
///
/// We split on `:` first to separate user+params from password,
/// then split the user portion on `;` to separate user from user-params.
fn parse_userinfo(s: &str) -> UserinfoResult {
    let err = |msg: &str| ParseSipUriError(msg.to_string());

    // Split user(+params) from password on first `:`
    // But `:` is in user-unreserved for SIP! The RFC ABNF says:
    //   userinfo = (user / telephone-subscriber) [":" password] "@"
    //   user = 1*(unreserved / escaped / user-unreserved)
    // And user-unreserved does NOT include `:` — that's the password delimiter.
    // Looking at the ABNF more carefully: user-unreserved = "&"/"="/"+"/"$"/","/";"/"?"/"/"
    // So `:` is NOT user-unreserved. The first `:` splits user from password.
    let (user_and_params, password) = if let Some(colon_pos) = s.find(':') {
        let pwd = &s[colon_pos + 1..];
        (&s[..colon_pos], Some(parse::canonize_password(pwd)))
    } else {
        (s, None)
    };

    // Split user from user-params on first `;`
    // In the userinfo, `;` separates user-params (used for tel: style params
    // like cpc=emergency). The user part itself is before the first `;`.
    if let Some(semi_pos) = user_and_params.find(';') {
        let user_part = &user_and_params[..semi_pos];
        let params_str = &user_and_params[semi_pos + 1..];

        if user_part.is_empty() {
            return Err(err("empty user before ';'"));
        }

        let user = parse::canonize_user(user_part);
        let user_params =
            params::parse_user_params(params_str).map_err(|e| err(&format!("user param: {e}")))?;

        Ok((Some(user), user_params, password))
    } else if user_and_params.is_empty() {
        Ok((None, Vec::new(), password))
    } else {
        let user = parse::canonize_user(user_and_params);
        Ok((Some(user), Vec::new(), password))
    }
}

/// Parse host, optional port, URI params, and headers from the portion after `@` (or after scheme: if no userinfo).
fn parse_hostport_params_headers(s: &str) -> HostportResult {
    let err = |msg: &str| ParseSipUriError(msg.to_string());

    // Parse host
    let (host, consumed) = Host::parse_from_uri(s).map_err(|e| err(&e))?;

    let rest = &s[consumed..];

    // Parse optional port
    let (port, rest) = if let Some(rest) = rest.strip_prefix(':') {
        // Port: digits until `;`, `?`, `#`, `>`, or end
        let end = rest
            .find([';', '?', '#', '>'])
            .unwrap_or(rest.len());
        let port_str = &rest[..end];

        if port_str.is_empty() {
            // Empty port is valid per sofia-sip (e.g., "sip:host:")
            (None, &rest[end..])
        } else {
            let port: u16 = port_str
                .parse()
                .map_err(|_| err(&format!("invalid port '{port_str}'")))?;
            (Some(port), &rest[end..])
        }
    } else {
        (None, rest)
    };

    // Strip fragment (#...) from the end before parsing params/headers
    let (rest, fragment) = if let Some(hash_pos) = rest.find('#') {
        let frag = &rest[hash_pos + 1..];
        let frag = if frag.is_empty() {
            None
        } else {
            Some(frag.to_string())
        };
        (&rest[..hash_pos], frag)
    } else {
        (rest, None)
    };

    // Parse URI params (after `;`) and headers (after `?`)
    let (params_str, headers_str) = if let Some(rest) = rest.strip_prefix(';') {
        // Split params from headers on `?`
        if let Some(q_pos) = rest.find('?') {
            (&rest[..q_pos], Some(&rest[q_pos + 1..]))
        } else {
            (rest, None)
        }
    } else if let Some(rest) = rest.strip_prefix('?') {
        ("", Some(rest))
    } else if rest.is_empty() {
        ("", None)
    } else {
        return Err(err(&format!(
            "unexpected character after host/port: '{rest}'"
        )));
    };

    let uri_params =
        params::parse_params(params_str).map_err(|e| err(&format!("URI param: {e}")))?;

    let headers = if let Some(h) = headers_str {
        params::parse_headers(h).map_err(|e| err(&format!("header: {e}")))?
    } else {
        Vec::new()
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
        }

        // Host
        self.host
            .fmt_uri(f)?;

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
        assert_eq!(uri.header("From"), Some("foo@bar"));
        assert_eq!(uri.header("To"), Some("bar@baz"));
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
            "sips:user:pass@host:32;param=1?From=foo@bar&To=bar@baz"
        );
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
