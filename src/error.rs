use std::fmt;

/// Error returned when parsing a SIP or SIPS URI fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSipUriError(pub String);

impl fmt::Display for ParseSipUriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid SIP URI: {}", self.0)
    }
}

impl std::error::Error for ParseSipUriError {}

/// Error returned when parsing a tel: URI fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTelUriError(pub String);

impl fmt::Display for ParseTelUriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid tel URI: {}", self.0)
    }
}

impl std::error::Error for ParseTelUriError {}

/// Error returned when parsing a [`Uri`](crate::Uri) fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseUriError(pub String);

impl fmt::Display for ParseUriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URI: {}", self.0)
    }
}

impl std::error::Error for ParseUriError {}

impl From<ParseSipUriError> for ParseUriError {
    fn from(e: ParseSipUriError) -> Self {
        ParseUriError(e.0)
    }
}

impl From<ParseTelUriError> for ParseUriError {
    fn from(e: ParseTelUriError) -> Self {
        ParseUriError(e.0)
    }
}

/// Error returned when parsing a URN fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseUrnError(pub String);

impl fmt::Display for ParseUrnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URN: {}", self.0)
    }
}

impl std::error::Error for ParseUrnError {}

impl From<ParseUrnError> for ParseUriError {
    fn from(e: ParseUrnError) -> Self {
        ParseUriError(e.0)
    }
}

/// Error returned when parsing a [`NameAddr`](crate::NameAddr) fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNameAddrError(pub String);

impl fmt::Display for ParseNameAddrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid name-addr: {}", self.0)
    }
}

impl std::error::Error for ParseNameAddrError {}

impl From<ParseUriError> for ParseNameAddrError {
    fn from(e: ParseUriError) -> Self {
        ParseNameAddrError(e.0)
    }
}
