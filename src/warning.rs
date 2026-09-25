use std::fmt;

use crate::parse::validate_pct_encoded;

/// A parse result together with the non-conformance found on the way.
///
/// Returned by the `parse_with_warnings` constructors. `value` is what
/// [`FromStr`](std::str::FromStr) returns for the same input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Parsed<T> {
    /// The parsed value.
    pub value: T,
    /// Grammar breaches the parser accepted, in input order.
    pub warnings: Vec<ParseWarning>,
}

impl<T> Parsed<T> {
    /// Whether any warning was raised.
    pub fn has_warnings(&self) -> bool {
        !self
            .warnings
            .is_empty()
    }
}

/// A grammar breach the parser accepted rather than rejected.
///
/// Names where the breach is, never what text it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ParseWarning {
    /// URI component the breach is in.
    pub component: Component,
    /// What is wrong with it.
    pub code: WarningCode,
    /// Byte offset into the parsed input, when one points at the breach.
    pub position: Option<usize>,
    /// Whether the parsed value still carries what was sent.
    pub kind: WarningKind,
}

impl fmt::Display for ParseWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            self.component
                .as_str(),
            self.code
                .describe()
        )?;
        if let Some(pos) = self.position {
            write!(f, " at byte {pos}")?;
        }
        Ok(())
    }
}

/// URI component a [`ParseWarning`] refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Component {
    /// The scheme before the first `:`.
    Scheme,
    /// SIP user part.
    User,
    /// SIP user-param, inside the userinfo.
    UserParam,
    /// SIP password.
    Password,
    /// Host.
    Host,
    /// Port.
    Port,
    /// URI parameter after the host, or a tel: parameter.
    Param,
    /// SIP URI header after `?`.
    Header,
    /// Fragment after `#`.
    Fragment,
    /// tel: telephone number.
    Number,
    /// URN r-component (`?+`).
    RComponent,
    /// URN q-component (`?=`).
    QComponent,
    /// URN f-component (`#`).
    FComponent,
}

impl Component {
    /// Stable lowercase name, for logs and machine consumers.
    pub fn as_str(self) -> &'static str {
        match self {
            Component::Scheme => "scheme",
            Component::User => "user",
            Component::UserParam => "uparam",
            Component::Password => "password",
            Component::Host => "host",
            Component::Port => "port",
            Component::Param => "param",
            Component::Header => "header",
            Component::Fragment => "fragment",
            Component::Number => "number",
            Component::RComponent => "r-component",
            Component::QComponent => "q-component",
            Component::FComponent => "f-component",
        }
    }
}

/// What a [`ParseWarning`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WarningCode {
    /// A character outside the component's character set, kept literally.
    InvalidChar,
    /// A `%` not followed by two hex digits, kept literally.
    MalformedEscape,
    /// A parameter with no name before `=`.
    EmptyName,
    /// An empty `;` or `&` segment, dropped.
    EmptySegment,
    /// A password with no user before it.
    PasswordWithoutUser,
    /// A port written with a `+` sign.
    SignedPort,
    /// A `:` with no port after it, dropped.
    EmptyPort,
    /// A host label that is empty or starts or ends with `-`.
    InvalidHostLabel,
    /// A hostname whose last label does not start with a letter.
    NumericToplabel,
    /// A hostname written with percent-escapes.
    EscapedHost,
    /// A fragment on a URI whose grammar defines none.
    UnexpectedFragment,
    /// A `#` with nothing after it, dropped.
    EmptyFragment,
    /// A user part containing `?`, likely a header value whose `@` was taken
    /// as the userinfo delimiter.
    HeaderShapedUser,
    /// A local tel: number without `phone-context`.
    MissingPhoneContext,
    /// A URN `?+` or `?=` with nothing after it.
    EmptyComponent,
    /// A scheme outside the RFC 3986 grammar.
    InvalidScheme,
}

impl WarningCode {
    /// Stable kebab-case name, for logs and machine consumers.
    pub fn as_str(self) -> &'static str {
        match self {
            WarningCode::InvalidChar => "invalid-char",
            WarningCode::MalformedEscape => "malformed-escape",
            WarningCode::EmptyName => "empty-name",
            WarningCode::EmptySegment => "empty-segment",
            WarningCode::PasswordWithoutUser => "password-without-user",
            WarningCode::SignedPort => "signed-port",
            WarningCode::EmptyPort => "empty-port",
            WarningCode::InvalidHostLabel => "invalid-host-label",
            WarningCode::NumericToplabel => "numeric-toplabel",
            WarningCode::EscapedHost => "escaped-host",
            WarningCode::UnexpectedFragment => "unexpected-fragment",
            WarningCode::EmptyFragment => "empty-fragment",
            WarningCode::HeaderShapedUser => "header-shaped-user",
            WarningCode::MissingPhoneContext => "missing-phone-context",
            WarningCode::EmptyComponent => "empty-component",
            WarningCode::InvalidScheme => "invalid-scheme",
        }
    }

    fn describe(self) -> &'static str {
        match self {
            WarningCode::InvalidChar => "invalid character",
            WarningCode::MalformedEscape => "malformed percent-escape",
            WarningCode::EmptyName => "empty parameter name",
            WarningCode::EmptySegment => "empty segment dropped",
            WarningCode::PasswordWithoutUser => "password without user",
            WarningCode::SignedPort => "signed port",
            WarningCode::EmptyPort => "empty port dropped",
            WarningCode::InvalidHostLabel => "invalid host label",
            WarningCode::NumericToplabel => "last host label does not start with a letter",
            WarningCode::EscapedHost => "percent-escaped hostname",
            WarningCode::UnexpectedFragment => "fragment not defined for this scheme",
            WarningCode::EmptyFragment => "empty fragment dropped",
            WarningCode::HeaderShapedUser => "user part contains '?'",
            WarningCode::MissingPhoneContext => "local number without phone-context",
            WarningCode::EmptyComponent => "empty component",
            WarningCode::InvalidScheme => "invalid scheme",
        }
    }

    fn kind(self) -> WarningKind {
        match self {
            WarningCode::EmptySegment | WarningCode::EmptyPort | WarningCode::EmptyFragment => {
                WarningKind::Lost
            }
            _ => WarningKind::Recovered,
        }
    }
}

/// Whether the parsed value still carries what the input sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WarningKind {
    /// Kept as sent; re-serializing reproduces it.
    Recovered,
    /// Dropped; re-serializing omits it.
    Lost,
}

/// Collects warnings with positions relative to the whole parsed input.
pub(crate) struct Warnings<'a> {
    input: &'a str,
    list: Vec<ParseWarning>,
}

impl<'a> Warnings<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Warnings {
            input,
            list: Vec::new(),
        }
    }

    /// Record `code` at byte `at` of `part`, a slice borrowed from the input.
    pub(crate) fn push(&mut self, component: Component, code: WarningCode, part: &str, at: usize) {
        let position = (part.as_ptr() as usize)
            .checked_sub(
                self.input
                    .as_ptr() as usize,
            )
            .map(|start| start + at)
            .filter(|&pos| {
                pos <= self
                    .input
                    .len()
            });
        self.list
            .push(ParseWarning {
                component,
                code,
                position,
                kind: code.kind(),
            });
    }

    /// Warn on the first byte of `part` outside `allowed` or a well-formed escape.
    pub(crate) fn charset(&mut self, component: Component, part: &str, allowed: fn(u8) -> bool) {
        if let Err(pos) = validate_pct_encoded(part, allowed) {
            let code = if part.as_bytes()[pos] == b'%' {
                WarningCode::MalformedEscape
            } else {
                WarningCode::InvalidChar
            };
            self.push(component, code, part, pos);
        }
    }

    pub(crate) fn finish<T>(self, value: T) -> Parsed<T> {
        Parsed {
            value,
            warnings: self.list,
        }
    }
}
