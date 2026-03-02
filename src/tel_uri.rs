use std::fmt;
use std::str::FromStr;

use crate::error::ParseTelUriError;
use crate::params;

/// tel: URI per RFC 3966.
///
/// Represents a telephone number with optional parameters.
/// Global numbers start with `+`, local numbers require a `phone-context` parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TelUri {
    number: String,
    params: Vec<(String, Option<String>)>,
}

impl TelUri {
    /// Create a new tel: URI with the given number.
    ///
    /// The number should include `+` prefix for global numbers.
    pub fn new(number: impl Into<String>) -> Self {
        TelUri {
            number: number.into(),
            params: Vec::new(),
        }
    }

    /// Add a parameter.
    pub fn with_param(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        self.params
            .push((name.into(), value));
        self
    }

    /// The telephone number (including `+` for global numbers, including visual separators).
    pub fn number(&self) -> &str {
        &self.number
    }

    /// Whether this is a global number (starts with `+`).
    pub fn is_global(&self) -> bool {
        self.number
            .starts_with('+')
    }

    /// Parameters.
    pub fn params(&self) -> &[(String, Option<String>)] {
        &self.params
    }

    /// Look up a parameter by name (case-insensitive).
    pub fn param(&self, name: &str) -> Option<&Option<String>> {
        params::find_param(&self.params, name)
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
    type Err = ParseTelUriError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let err = |msg: &str| ParseTelUriError(msg.to_string());

        // Strip scheme
        let rest = input
            .strip_prefix("tel:")
            .or_else(|| input.strip_prefix("TEL:"))
            .or_else(|| {
                let colon = input.find(':')?;
                if input[..colon].eq_ignore_ascii_case("tel") {
                    Some(&input[colon + 1..])
                } else {
                    None
                }
            })
            .ok_or_else(|| err("missing 'tel:' scheme"))?;

        if rest.is_empty() {
            return Err(err("empty telephone number"));
        }

        // Split number from params at first `;`
        let (number_str, params_str) = if let Some(semi) = rest.find(';') {
            (&rest[..semi], Some(&rest[semi + 1..]))
        } else {
            (rest, None)
        };

        // Validate number
        if number_str.is_empty() {
            return Err(err("empty telephone number"));
        }

        let number_bytes = number_str.as_bytes();

        if number_bytes[0] == b'+' {
            // Global number: + followed by phonedigits with at least one DIGIT
            if number_bytes.len() < 2 {
                return Err(err("global number must have digits after '+'"));
            }
            let digits = &number_bytes[1..];
            if !digits
                .iter()
                .all(|&b| is_phonedigit(b))
            {
                return Err(err("invalid character in global number"));
            }
            if !digits
                .iter()
                .any(|b| b.is_ascii_digit())
            {
                return Err(err("global number must contain at least one digit"));
            }
        } else {
            // Local number: phonedigit-hex chars, must contain at least one
            // HEXDIG or * or #
            if !number_bytes
                .iter()
                .all(|&b| is_phonedigit_hex(b))
            {
                return Err(err("invalid character in local number"));
            }
            if !number_bytes
                .iter()
                .any(|&b| b.is_ascii_hexdigit() || matches!(b, b'*' | b'#'))
            {
                return Err(err(
                    "local number must contain at least one hex digit, *, or #",
                ));
            }
        }

        let params = if let Some(p) = params_str {
            params::parse_params(p).map_err(|e| err(&format!("param: {e}")))?
        } else {
            Vec::new()
        };

        Ok(TelUri {
            number: number_str.to_string(),
            params,
        })
    }
}

impl fmt::Display for TelUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tel:{}", self.number)?;
        params::format_params(&self.params, f)?;
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
        assert_eq!(uri.number(), "+12345678");
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
        assert_eq!(uri.number(), "+12345678");
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
        assert_eq!(uri.number(), "911");
        assert!(!uri.is_global());
    }

    #[test]
    fn parse_local_with_context() {
        let uri: TelUri = "tel:1411;phone-context=example.com"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), "1411");
        assert_eq!(
            uri.param("phone-context"),
            Some(&Some("example.com".into()))
        );
    }

    #[test]
    fn parse_visual_separators() {
        let uri: TelUri = "tel:+1.245.623-57"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), "+1.245.623-57");
    }

    #[test]
    fn parse_dtmf_local() {
        let uri: TelUri = "tel:*67"
            .parse()
            .unwrap();
        assert_eq!(uri.number(), "*67");
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
    fn empty_number_fails() {
        assert!("tel:"
            .parse::<TelUri>()
            .is_err());
    }

    #[test]
    fn plus_only_fails() {
        assert!("tel:+"
            .parse::<TelUri>()
            .is_err());
    }

    #[test]
    fn param_without_value() {
        let uri: TelUri = "tel:+12345678;oli"
            .parse()
            .unwrap();
        assert_eq!(uri.param("oli"), Some(&None));
    }

    #[test]
    fn builder() {
        let uri = TelUri::new("+15551234567").with_param("cpc", Some("emergency".into()));
        assert_eq!(uri.to_string(), "tel:+15551234567;cpc=emergency");
    }
}
