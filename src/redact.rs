use std::fmt;

use sip_uri_types::Pairs;
use sip_uri_types::SipUri;
use sip_uri_types::TelUri;
use sip_uri_types::Uri;

const MASK: &str = "***";

/// How the user part, or a tel: number, is rendered by [`Redaction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserMask {
    /// The whole userinfo, user-params and password included, becomes `***`.
    Full,
    /// Every digit but the last `n` becomes `x`; other characters stay. Suits
    /// telephone numbers; a name has no digits to mask.
    KeepLast(usize),
    /// The user part is shown as Display writes it.
    Visible,
}

/// What a redacted rendering masks.
///
/// [`Redaction::default`] masks the whole userinfo, or a tel: number, and
/// nothing else. A password is `***` under every [`UserMask`]. URN and
/// unrecognized URIs render unmasked, so an identifier such as an IMEI in a
/// URN NSS is the caller's to keep out of logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Redaction<'a> {
    user: UserMask,
    drop_headers: bool,
    params: &'a [&'a str],
}

impl Default for Redaction<'_> {
    fn default() -> Self {
        Redaction {
            user: UserMask::Full,
            drop_headers: false,
            params: &[],
        }
    }
}

impl<'a> Redaction<'a> {
    /// Set how the user part or tel: number is rendered.
    pub fn user(mut self, mask: UserMask) -> Self {
        self.user = mask;
        self
    }

    /// How the user part or tel: number is rendered.
    pub fn user_mask(&self) -> UserMask {
        self.user
    }

    /// Leave URI headers out of the rendering.
    pub fn drop_headers(mut self) -> Self {
        self.drop_headers = true;
        self
    }

    /// Render the values of these params (case-insensitive) as `***`.
    pub fn params(mut self, names: &'a [&'a str]) -> Self {
        self.params = names;
        self
    }

    fn masks_param(&self, name: &str) -> bool {
        self.params
            .iter()
            .any(|p| p.eq_ignore_ascii_case(name))
    }
}

/// [`fmt::Display`] adapter returned by [`UriRedact::redacted`].
#[derive(Debug, Clone, Copy)]
pub struct Redacted<'a, T> {
    uri: &'a T,
    how: Redaction<'a>,
}

/// Rendering for logs, for [`SipUri`], [`TelUri`] and [`Uri`].
pub trait UriRedact: Sized {
    /// Render for logs, masking what `how` names.
    ///
    /// ```
    /// use sip_uri::{Redaction, SipUri, UriParse, UriRedact, UserMask};
    ///
    /// let uri = SipUri::parse("sip:+15551234567;cpc=ordinary:pw@example.com").unwrap();
    /// assert_eq!(uri.redacted(Redaction::default()).to_string(), "sip:***@example.com");
    /// assert_eq!(
    ///     uri.redacted(Redaction::default().user(UserMask::KeepLast(4))).to_string(),
    ///     "sip:+xxxxxxx4567;cpc=ordinary:***@example.com"
    /// );
    /// ```
    fn redacted<'a>(&'a self, how: Redaction<'a>) -> Redacted<'a, Self>;
}

impl UriRedact for SipUri {
    fn redacted<'a>(&'a self, how: Redaction<'a>) -> Redacted<'a, Self> {
        Redacted { uri: self, how }
    }
}

impl UriRedact for TelUri {
    fn redacted<'a>(&'a self, how: Redaction<'a>) -> Redacted<'a, Self> {
        Redacted { uri: self, how }
    }
}

impl UriRedact for Uri {
    fn redacted<'a>(&'a self, how: Redaction<'a>) -> Redacted<'a, Self> {
        Redacted { uri: self, how }
    }
}

fn write_masked_user(f: &mut fmt::Formatter<'_>, user: &str, mask: UserMask) -> fmt::Result {
    match mask {
        UserMask::Full => f.write_str(MASK),
        UserMask::Visible => f.write_str(user),
        UserMask::KeepLast(n) => {
            let digits = user
                .bytes()
                .filter(u8::is_ascii_digit)
                .count();
            let mut seen = 0;
            for c in user.chars() {
                if c.is_ascii_digit() {
                    seen += 1;
                    if seen <= digits.saturating_sub(n) {
                        f.write_str("x")?;
                        continue;
                    }
                }
                write!(f, "{c}")?;
            }
            Ok(())
        }
    }
}

fn write_params(f: &mut fmt::Formatter<'_>, params: Pairs<'_>, how: &Redaction<'_>) -> fmt::Result {
    for (name, value) in params {
        write!(f, ";{name}")?;
        match value {
            Some(_) if how.masks_param(name) => write!(f, "={MASK}")?,
            Some(v) => write!(f, "={v}")?,
            None => {}
        }
    }
    Ok(())
}

impl fmt::Display for Redacted<'_, SipUri> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (uri, how) = (self.uri, &self.how);
        if let Some(scheme) = uri.scheme() {
            write!(f, "{scheme}:")?;
        }
        if uri
            .user()
            .is_some()
            || uri
                .password()
                .is_some()
        {
            if how.user == UserMask::Full {
                write!(f, "{MASK}@")?;
            } else {
                if let Some(user) = uri.user() {
                    write_masked_user(f, user, how.user)?;
                    write_params(
                        f,
                        uri.user_params()
                            .iter(),
                        how,
                    )?;
                }
                if uri
                    .password()
                    .is_some()
                {
                    write!(f, ":{MASK}")?;
                }
                f.write_str("@")?;
            }
        }
        if let Some(host) = uri.host() {
            write!(f, "{host}")?;
        }
        if let Some(port) = uri.port() {
            write!(f, ":{port}")?;
        }
        write_params(
            f,
            uri.params()
                .iter(),
            how,
        )?;
        if !how.drop_headers {
            for (i, (name, value)) in uri
                .headers()
                .iter()
                .enumerate()
            {
                let sep = if i == 0 { '?' } else { '&' };
                write!(f, "{sep}{name}")?;
                if let Some(value) = value {
                    write!(f, "={value}")?;
                }
            }
        }
        if let Some(frag) = uri.fragment() {
            write!(f, "#{frag}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Redacted<'_, TelUri> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (uri, how) = (self.uri, &self.how);
        f.write_str("tel:")?;
        if let Some(number) = uri.number() {
            write_masked_user(f, number, how.user)?;
        }
        write_params(
            f,
            uri.params()
                .iter(),
            how,
        )?;
        if let Some(frag) = uri.fragment() {
            write!(f, "#{frag}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Redacted<'_, Uri> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.uri {
            Uri::Sip(u) => write!(f, "{}", u.redacted(self.how)),
            Uri::Tel(u) => write!(f, "{}", u.redacted(self.how)),
            Uri::Urn(u) => write!(f, "{u}"),
            Uri::Other(o) => write!(f, "{o}"),
        }
    }
}
