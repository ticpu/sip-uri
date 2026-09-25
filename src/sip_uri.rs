use std::fmt;

use crate::canon;
use crate::host::Host;
use crate::params::{self, Params};

/// SIP or SIPS URI per RFC 3261 §19.
///
/// Supports the full grammar including user-params (`;` within userinfo),
/// password, IPv6 hosts, URI parameters, and headers. A scheme or host that
/// is missing or unreadable is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SipUri {
    scheme: Option<Scheme>,
    user: Option<String>,
    user_params: Vec<(String, Option<String>)>,
    password: Option<String>,
    host: Option<Host>,
    port: Option<u16>,
    params: Vec<(String, Option<String>)>,
    headers: Vec<(String, String)>,
    fragment: Option<String>,
}

/// The components of a [`SipUri`], each canonized on conversion.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SipUriParts {
    /// `sip` or `sips`.
    pub scheme: Option<Scheme>,
    /// The user part, without user-params or password.
    pub user: Option<String>,
    /// Parameters within the userinfo, before `@`.
    pub user_params: Vec<(String, Option<String>)>,
    /// The password.
    pub password: Option<String>,
    /// The host.
    pub host: Option<Host>,
    /// The port.
    pub port: Option<u16>,
    /// URI parameters after the host.
    pub params: Vec<(String, Option<String>)>,
    /// URI headers after `?`.
    pub headers: Vec<(String, String)>,
    /// The fragment after `#`.
    pub fragment: Option<String>,
}

impl From<SipUriParts> for SipUri {
    fn from(p: SipUriParts) -> Self {
        let user_params = params::canonize_pairs(p.user_params, canon::canonize_user_param);
        SipUri {
            scheme: p.scheme,
            user: hold_user(
                p.user
                    .as_deref()
                    .map(canon::canonize_user),
                &user_params,
            ),
            user_params,
            password: p
                .password
                .as_deref()
                .map(canon::canonize_password),
            host: p
                .host
                .map(Host::normalized)
                .filter(|h| !matches!(h, Host::Hostname(name) if name.is_empty())),
            port: p.port,
            params: params::canonize_pairs(p.params, canon::canonize_param),
            headers: p
                .headers
                .iter()
                .map(|(name, value)| canon::canonize_header(name, value))
                .collect(),
            fragment: p
                .fragment
                .as_deref()
                .filter(|f| !f.is_empty())
                .map(canon::canonize_sip_fragment),
        }
    }
}

/// An empty user prints as a bare `@`, which parses as no userinfo, unless
/// user-params follow it; user-params print only after a user.
fn hold_user(user: Option<String>, user_params: &Params) -> Option<String> {
    match user {
        Some(u) if u.is_empty() && user_params.is_empty() => None,
        None if !user_params.is_empty() => Some(String::new()),
        user => user,
    }
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
        SipUriParts {
            scheme: Some(Scheme::Sip),
            host: Some(host),
            ..Default::default()
        }
        .into()
    }

    /// The components, in canonical form.
    pub fn into_parts(self) -> SipUriParts {
        SipUriParts {
            scheme: self.scheme,
            user: self.user,
            user_params: self.user_params,
            password: self.password,
            host: self.host,
            port: self.port,
            params: self.params,
            headers: self.headers,
            fragment: self.fragment,
        }
    }

    /// Set the URI scheme.
    pub fn with_scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = Some(scheme);
        self
    }

    /// Set the user part, canonized like parsed text: a delimiter or byte
    /// outside the user grammar is escaped, so it cannot add user-params.
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = hold_user(Some(canon::canonize_user(&user.into())), &self.user_params);
        self
    }

    /// Replace all user-params (parameters within the userinfo, before `@`),
    /// canonized as [`SipUri::with_user_param`] does.
    pub fn with_user_params(mut self, params: Vec<(String, Option<String>)>) -> Self {
        self.user_params = params::canonize_pairs(params, canon::canonize_user_param);
        self.user = hold_user(self.user, &self.user_params);
        self
    }

    /// Add a single user-param (parameter within the userinfo, before `@`),
    /// escaping any delimiter in the name or value.
    pub fn with_user_param(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        params::push_pair(
            &mut self.user_params,
            &name.into(),
            value.as_deref(),
            canon::canonize_user_param,
        );
        self.user = hold_user(self.user, &self.user_params);
        self
    }

    /// Set the password, escaping any delimiter in it.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(canon::canonize_password(&password.into()));
        self
    }

    /// Set the port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Add a URI parameter, escaping any delimiter in the name or value.
    pub fn with_param(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        params::push_pair(
            &mut self.params,
            &name.into(),
            value.as_deref(),
            canon::canonize_param,
        );
        self
    }

    /// Add a header, escaping any delimiter in the name or value.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .push(canon::canonize_header(&name.into(), &value.into()));
        self
    }

    /// Set the fragment component, escaping any delimiter in it.
    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        let fragment = fragment.into();
        self.fragment = (!fragment.is_empty()).then(|| canon::canonize_sip_fragment(&fragment));
        self
    }

    /// The URI scheme (`sip` or `sips`), `None` when the input had none.
    pub fn scheme(&self) -> Option<Scheme> {
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

    /// The host, `None` when missing or unreadable.
    pub fn host(&self) -> Option<&Host> {
        self.host
            .as_ref()
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
    pub fn param(&self, name: &str) -> Option<Option<&str>> {
        params::find_param(&self.params, name)
    }

    /// Look up a user-param by name (case-insensitive).
    pub fn user_param(&self, name: &str) -> Option<Option<&str>> {
        params::find_param(&self.user_params, name)
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

    /// Convenience: `user@host:port`, with each absent part left out.
    pub fn user_host(&self) -> String {
        let mut s = String::new();
        if let Some(ref u) = self.user {
            s.push_str(u);
            s.push('@');
        }
        if let Some(ref host) = self.host {
            s.push_str(&host.to_string());
        }
        if let Some(p) = self.port {
            s.push(':');
            s.push_str(&p.to_string());
        }
        s
    }
}

impl fmt::Display for SipUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scheme) = self.scheme {
            write!(f, "{scheme}:")?;
        }

        if let Some(ref user) = self.user {
            write!(f, "{user}")?;
            params::format_params(&self.user_params, f)?;
            if let Some(ref pwd) = self.password {
                write!(f, ":{pwd}")?;
            }
            write!(f, "@")?;
        } else if let Some(ref pwd) = self.password {
            write!(f, ":{pwd}@")?;
        }

        if let Some(ref host) = self.host {
            write!(f, "{host}")?;
        }
        if let Some(port) = self.port {
            write!(f, ":{port}")?;
        }
        params::format_params(&self.params, f)?;
        params::format_headers(&self.headers, f)?;
        if let Some(ref frag) = self.fragment {
            write!(f, "#{frag}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder() {
        let uri = SipUri::new(Host::Hostname("example.com".into()))
            .with_user("alice")
            .with_param("transport", Some("tcp".into()));
        assert_eq!(uri.to_string(), "sip:alice@example.com;transport=tcp");
    }

    #[test]
    fn parts_canonize_like_builders() {
        let uri = SipUri::from(SipUriParts {
            scheme: Some(Scheme::Sip),
            user: Some("a b;c".into()),
            host: Some(Host::Hostname("example.com".into())),
            params: vec![("x".into(), Some("a@b".into()))],
            ..Default::default()
        });
        let built = SipUri::new(Host::Hostname("example.com".into()))
            .with_user("a b;c")
            .with_param("x", Some("a@b".into()));
        assert_eq!(uri, built);
        assert_eq!(uri.user(), Some("a%20b%3Bc"));
        assert_eq!(uri.param("x"), Some(Some("a%40b")));
        assert_eq!(
            SipUri::from(
                uri.clone()
                    .into_parts()
            ),
            uri
        );
    }

    #[test]
    fn empty_components_are_absent() {
        let host = || Host::Hostname("example.com".into());
        let uri = SipUri::new(host())
            .with_user("")
            .with_param("", None)
            .with_fragment("");
        assert_eq!(uri, SipUri::new(host()));
        assert_eq!(uri.to_string(), "sip:example.com");
        assert_eq!(SipUri::new(Host::Hostname("".into())).host(), None);
    }

    #[test]
    fn dotted_quad_hostname_is_held_as_ipv4() {
        let built = SipUri::new(Host::Hostname("198.51.100.1".into()));
        assert_eq!(
            built.host(),
            Some(&Host::IPv4(std::net::Ipv4Addr::new(198, 51, 100, 1)))
        );
        let leading_zero = SipUri::new(Host::Hostname("198.051.100.1".into()));
        assert!(matches!(leading_zero.host(), Some(Host::Hostname(_))));
    }

    #[test]
    fn user_params_hold_an_empty_user() {
        let host = || Host::Hostname("example.com".into());
        let uri = SipUri::new(host())
            .with_user("")
            .with_user_param("cpc", Some("x".into()));
        assert_eq!(uri.user(), Some(""));
        assert_eq!(uri.to_string(), "sip:;cpc=x@example.com");
        let from_parts = SipUri::from(SipUriParts {
            scheme: Some(Scheme::Sip),
            user_params: vec![("cpc".into(), Some("x".into()))],
            host: Some(host()),
            ..Default::default()
        });
        assert_eq!(from_parts, uri);
        assert_eq!(
            uri.with_user_params(Vec::new())
                .user(),
            None
        );
    }
}
