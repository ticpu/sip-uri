use std::fmt;
use std::sync::Arc;

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

/// How the headers of a SIP URI are rendered by [`Redaction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeaderMask {
    /// Each value becomes `***`; names stay, and a header without a value
    /// stays its name alone.
    Values,
    /// Headers are shown as Display writes them.
    Visible,
    /// Headers are left out.
    Dropped,
}

/// What a redacted rendering masks.
///
/// [`Redaction::default`] masks the whole userinfo, or a tel: number, and
/// every URI header value, since headers such as `P-Asserted-Identity`
/// carry identities: `sip:***@example.com?Subject=***`. Params are shown
/// unless named in [`Redaction::params`]. A password is `***` under every
/// [`UserMask`]. URN and unrecognized URIs render unmasked, so an
/// identifier such as an IMEI in a URN NSS is the caller's to keep out of
/// logs.
///
/// A policy owns its data and clones cheaply, so one read from configuration
/// is built once and lent to every rendering:
///
/// ```
/// use sip_uri::{Redaction, SipUri, UriParse, UriRedact, UserMask};
///
/// struct Logger {
///     redaction: Redaction,
/// }
///
/// impl Logger {
///     fn new(masked_params: &[String]) -> Self {
///         let redaction = Redaction::default()
///             .user(UserMask::KeepLast(4))
///             .params(masked_params);
///         Logger { redaction }
///     }
///
///     fn line(&self, uri: &SipUri) -> String {
///         format!("call from {}", uri.redacted(&self.redaction))
///     }
/// }
///
/// let logger = Logger::new(&["participantid".to_string()]);
/// let uri = SipUri::parse("sip:+15551234567@example.com;ParticipantId=7").unwrap();
/// assert_eq!(
///     logger.line(&uri),
///     "call from sip:+xxxxxxx4567@example.com;ParticipantId=***"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Redaction {
    user: UserMask,
    headers: HeaderMask,
    params: Arc<[String]>,
}

impl Default for Redaction {
    fn default() -> Self {
        Redaction {
            user: UserMask::Full,
            headers: HeaderMask::Values,
            params: Arc::from([]),
        }
    }
}

impl Redaction {
    /// Set how the user part or tel: number is rendered.
    pub fn user(mut self, mask: UserMask) -> Self {
        self.user = mask;
        self
    }

    /// How the user part or tel: number is rendered.
    pub fn user_mask(&self) -> UserMask {
        self.user
    }

    /// Set how URI headers are rendered.
    pub fn headers(mut self, mask: HeaderMask) -> Self {
        self.headers = mask;
        self
    }

    /// How URI headers are rendered.
    pub fn header_mask(&self) -> HeaderMask {
        self.headers
    }

    /// Leave URI headers out of the rendering: [`HeaderMask::Dropped`].
    pub fn drop_headers(self) -> Self {
        self.headers(HeaderMask::Dropped)
    }

    /// Render the values of params with these names, compared
    /// case-insensitively, as `***`, in place of any set before.
    pub fn params<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.params = names
            .into_iter()
            .map(Into::into)
            .collect();
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
    how: &'a Redaction,
}

mod sealed {
    pub trait Sealed {}
}

/// Rendering for logs, for [`SipUri`], [`TelUri`] and [`Uri`].
pub trait UriRedact: Sized + sealed::Sealed {
    /// Render for logs, masking what `how` names.
    ///
    /// ```
    /// use sip_uri::{Redaction, SipUri, UriParse, UriRedact, UserMask};
    ///
    /// let uri = SipUri::parse("sip:+15551234567;cpc=ordinary:pw@example.com").unwrap();
    /// assert_eq!(uri.redacted(&Redaction::default()).to_string(), "sip:***@example.com");
    /// assert_eq!(
    ///     uri.redacted(&Redaction::default().user(UserMask::KeepLast(4))).to_string(),
    ///     "sip:+xxxxxxx4567;cpc=ordinary:***@example.com"
    /// );
    /// ```
    fn redacted<'a>(&'a self, how: &'a Redaction) -> Redacted<'a, Self>;
}

macro_rules! impl_uri_redact {
    ($($ty:ty),*) => {$(
        impl sealed::Sealed for $ty {}

        impl UriRedact for $ty {
            fn redacted<'a>(&'a self, how: &'a Redaction) -> Redacted<'a, Self> {
                Redacted { uri: self, how }
            }
        }
    )*};
}

impl_uri_redact!(SipUri, TelUri, Uri);

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

fn write_params(f: &mut fmt::Formatter<'_>, params: Pairs<'_>, how: &Redaction) -> fmt::Result {
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
        let (uri, how) = (self.uri, self.how);
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
        if how.headers != HeaderMask::Dropped {
            for (i, (name, value)) in uri
                .headers()
                .iter()
                .enumerate()
            {
                let sep = if i == 0 { '?' } else { '&' };
                write!(f, "{sep}{name}")?;
                match value {
                    Some(_) if how.headers == HeaderMask::Values => write!(f, "={MASK}")?,
                    Some(v) => write!(f, "={v}")?,
                    None => {}
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
        let (uri, how) = (self.uri, self.how);
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
