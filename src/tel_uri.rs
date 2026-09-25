use std::fmt;
use std::str::FromStr;

use crate::error::ParseError;
use crate::params;
use crate::parse::{self, SchemeSplit};
use crate::warning::{Component, Parsed, WarningCode, Warnings};

/// tel: URI per RFC 3966.
///
/// Represents a telephone number with optional parameters.
/// Global numbers start with `+`. A local number without the `phone-context`
/// parameter RFC 3966 requires is accepted with a warning, and a missing
/// number is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TelUri {
    number: Option<String>,
    params: Vec<(String, Option<String>)>,
    fragment: Option<String>,
}

impl TelUri {
    /// Create a new tel: URI with the given number.
    ///
    /// The number should include `+` prefix for global numbers.
    pub fn new(number: impl Into<String>) -> Self {
        TelUri {
            number: Some(number.into()),
            params: Vec::new(),
            fragment: None,
        }
    }

    /// Add a parameter.
    pub fn with_param(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        self.params
            .push((name.into(), value));
        self
    }

    /// The telephone number (including `+` for global numbers, including
    /// visual separators), `None` when the input had none.
    pub fn number(&self) -> Option<&str> {
        self.number
            .as_deref()
    }

    /// Whether this is a global number (starts with `+`).
    pub fn is_global(&self) -> bool {
        self.number
            .as_deref()
            .is_some_and(|n| n.starts_with('+'))
    }

    /// Parameters.
    pub fn params(&self) -> &[(String, Option<String>)] {
        &self.params
    }

    /// Look up a parameter by name (case-insensitive).
    pub fn param(&self, name: &str) -> Option<Option<&str>> {
        params::find_param(&self.params, name)
    }

    /// The fragment component (after `#`), if present.
    pub fn fragment(&self) -> Option<&str> {
        self.fragment
            .as_deref()
    }

    /// Set the fragment component.
    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        self.fragment = Some(fragment.into());
        self
    }
}

/// RFC 3966: `phonedigit = DIGIT / visual-separator`
/// `visual-separator = "-" / "." / "(" / ")"`
fn is_phonedigit(c: u8) -> bool {
    c.is_ascii_digit() || matches!(c, b'-' | b'.' | b'(' | b')')
}

/// RFC 3966: `phonedigit-hex = HEXDIG / "*" / "#" / visual-separator`
fn is_phonedigit_hex(c: u8) -> bool {
    c.is_ascii_hexdigit() || matches!(c, b'*' | b'#' | b'-' | b'.' | b'(' | b')')
}

impl FromStr for TelUri {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse_with_warnings(input).map(|parsed| parsed.value)
    }
}

impl TelUri {
    /// Parse, rejecting any grammar breach as [`ParseError::NonConformant`].
    pub fn parse_strict(input: &str) -> Result<Self, ParseError> {
        Self::parse_with_warnings(input)?.into_strict()
    }

    /// Parse, reporting accepted grammar breaches beside the value.
    ///
    /// Accepts exactly what [`FromStr`] accepts: everything except empty
    /// input and a scheme other than `tel`.
    pub fn parse_with_warnings(input: &str) -> Result<Parsed<Self>, ParseError> {
        if input.is_empty() {
            return Err(ParseError::Empty);
        }
        let mut warnings = Warnings::new(input);

        let rest = match parse::split_scheme(input) {
            SchemeSplit::Named(s, rest) if s.eq_ignore_ascii_case("tel") => rest,
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

        let (number_str, params_str) = match rest.split_once(';') {
            Some((number, params)) => (number, Some(params)),
            None => (rest, None),
        };

        // `#` is a phonedigit-hex, so a fragment can only follow the params.
        let (params_str, fragment) = match params_str.map(|p| (p, p.find('#'))) {
            Some((p, Some(hash_pos))) => {
                let frag = &p[hash_pos + 1..];
                let frag = if frag.is_empty() {
                    warnings.push(Component::Fragment, WarningCode::EmptyFragment, p, hash_pos);
                    None
                } else {
                    warnings.push(
                        Component::Fragment,
                        WarningCode::UnexpectedFragment,
                        p,
                        hash_pos,
                    );
                    Some(frag.to_string())
                };
                (Some(&p[..hash_pos]), frag)
            }
            Some((p, None)) => (Some(p), None),
            None => (None, None),
        };

        let params = match params_str {
            Some(p) => params::parse_params(p, &params::TEL_PARAMS, &mut warnings),
            None => Vec::new(),
        };

        let number = if number_str.is_empty() {
            warnings.push(Component::Number, WarningCode::MissingNumber, number_str, 0);
            None
        } else {
            warn_number(number_str, &params, &mut warnings);
            Some(number_str.to_string())
        };

        Ok(warnings.finish(TelUri {
            number,
            params,
            fragment,
        }))
    }
}

/// RFC 3966: a local number needs a HEXDIG, `*` or `#`.
fn is_local_digit(b: u8) -> bool {
    b.is_ascii_hexdigit() || matches!(b, b'*' | b'#')
}

fn warn_number(number: &str, params: &[(String, Option<String>)], warnings: &mut Warnings) {
    let global = number.strip_prefix('+');
    let digits = global.unwrap_or(number);
    type ByteClass = fn(u8) -> bool;
    let (allowed, is_digit): (ByteClass, ByteClass) = if global.is_some() {
        (is_phonedigit, |b| b.is_ascii_digit())
    } else {
        (is_phonedigit_hex, is_local_digit)
    };
    if let Some(pos) = digits
        .bytes()
        .position(|b| !allowed(b))
    {
        warnings.push(Component::Number, WarningCode::InvalidChar, digits, pos);
    }
    if !digits
        .bytes()
        .any(is_digit)
    {
        warnings.push(Component::Number, WarningCode::NoDigits, number, 0);
    }
    if !number.starts_with('+') && params::find_param(params, "phone-context").is_none() {
        warnings.push(
            Component::Number,
            WarningCode::MissingPhoneContext,
            number,
            0,
        );
    }
}

impl fmt::Display for TelUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tel:")?;
        if let Some(ref number) = self.number {
            write!(f, "{number}")?;
        }
        params::format_params(&self.params, f)?;
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
    fn parse_global() {
        let uri: TelUri = "tel:+12345678"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), Some("+12345678"));
        assert!(uri.is_global());
        assert!(uri
            .params()
            .is_empty());
    }

    #[test]
    fn parse_with_params() {
        let uri: TelUri = "tel:+12345678;param=1;param=2"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), Some("+12345678"));
        assert_eq!(
            uri.params()
                .len(),
            2
        );
    }

    #[test]
    fn parse_local_number() {
        let uri: TelUri = "tel:911"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), Some("911"));
        assert!(!uri.is_global());
    }

    #[test]
    fn parse_local_with_context() {
        let uri: TelUri = "tel:1411;phone-context=example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), Some("1411"));
        assert_eq!(uri.param("phone-context"), Some(Some("example.com")));
    }

    #[test]
    fn parse_visual_separators() {
        let uri: TelUri = "tel:+1.245.623-57"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), Some("+1.245.623-57"));
    }

    #[test]
    fn parse_dtmf_local() {
        let uri: TelUri = "tel:*67"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), Some("*67"));
    }

    #[test]
    fn display_roundtrip() {
        let input = "tel:+12345678;cpc=emergency;oli=0";
        let uri: TelUri = input
            .parse()
            .unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn empty_number_is_none() {
        let parsed = TelUri::parse_with_warnings("tel:").unwrap();
        assert_eq!(
            parsed
                .value
                .number(),
            None
        );
        assert_eq!(parsed.warnings[0].code, WarningCode::MissingNumber);
    }

    #[test]
    fn plus_only_has_no_digits() {
        let parsed = TelUri::parse_with_warnings("tel:+").unwrap();
        assert_eq!(
            parsed
                .value
                .number(),
            Some("+")
        );
        assert_eq!(parsed.warnings[0].code, WarningCode::NoDigits);
    }

    #[test]
    fn empty_input_fails() {
        assert!(""
            .parse::<TelUri>()
            .is_err());
    }

    #[test]
    fn param_without_value() {
        let uri: TelUri = "tel:+12345678;oli"
            .parse()
            .unwrap();
        assert_eq!(uri.param("oli"), Some(None));
    }

    #[test]
    fn builder() {
        let uri = TelUri::new("+15551234567").with_param("cpc", Some("emergency".into()));
        assert_eq!(uri.to_string(), "tel:+15551234567;cpc=emergency");
    }
}
