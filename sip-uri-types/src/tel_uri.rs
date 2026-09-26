use std::fmt;

use crate::canon;
use crate::params;

/// tel: URI per RFC 3966.
///
/// Represents a telephone number with optional parameters.
/// Global numbers start with `+`. A local number without the `phone-context`
/// parameter RFC 3966 requires is still a value, and a missing number is
/// `None`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(from = "TelUriParts", into = "TelUriParts")
)]
#[non_exhaustive]
pub struct TelUri {
    number: Option<String>,
    params: Vec<(String, Option<String>)>,
    fragment: Option<String>,
}

/// The components of a [`TelUri`], each canonized on conversion.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
#[non_exhaustive]
pub struct TelUriParts {
    /// The telephone number, `+` first for a global one.
    pub number: Option<String>,
    /// Parameters after the number.
    pub params: Vec<(String, Option<String>)>,
    /// The fragment after `#`.
    pub fragment: Option<String>,
}

impl From<TelUriParts> for TelUri {
    fn from(p: TelUriParts) -> Self {
        TelUri {
            number: p
                .number
                .as_deref()
                .filter(|n| !n.is_empty())
                .map(canon::canonize_tel_number),
            params: params::canonize_pairs(p.params, canon::canonize_param),
            fragment: p
                .fragment
                .as_deref()
                .filter(|f| !f.is_empty())
                .map(canon::canonize_fragment),
        }
    }
}

impl From<TelUri> for TelUriParts {
    fn from(uri: TelUri) -> Self {
        uri.into_parts()
    }
}

impl TelUri {
    /// Create a new tel: URI with the given number, `+` first for a global
    /// one. A delimiter or byte outside the number grammar is escaped.
    pub fn new(number: impl Into<String>) -> Self {
        TelUriParts {
            number: Some(number.into()),
            ..Default::default()
        }
        .into()
    }

    /// The components, in canonical form.
    pub fn into_parts(self) -> TelUriParts {
        TelUriParts {
            number: self.number,
            params: self.params,
            fragment: self.fragment,
        }
    }

    /// Add a parameter, escaping any delimiter in the name or value.
    pub fn with_param(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        params::push_pair(
            &mut self.params,
            &name.into(),
            value.as_deref(),
            canon::canonize_param,
        );
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

    /// Set the fragment component, escaping any delimiter in it.
    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        let fragment = fragment.into();
        self.fragment = (!fragment.is_empty()).then(|| canon::canonize_fragment(&fragment));
        self
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
    fn builder() {
        let uri = TelUri::new("+15551234567").with_param("cpc", Some("emergency".into()));
        assert_eq!(uri.to_string(), "tel:+15551234567;cpc=emergency");
    }

    #[test]
    fn parts_canonize_like_builders() {
        let uri = TelUri::from(TelUriParts {
            number: Some("+1555 123".into()),
            params: vec![("a;b".into(), None)],
            ..Default::default()
        });
        assert_eq!(uri, TelUri::new("+1555 123").with_param("a;b", None));
        assert_eq!(uri.number(), Some("+1555%20123"));
        assert_eq!(
            TelUri::from(
                uri.clone()
                    .into_parts()
            ),
            uri
        );
    }

    #[test]
    fn empty_components_are_absent() {
        let uri = TelUri::new("")
            .with_param("", None)
            .with_fragment("");
        assert_eq!(uri, TelUri::from(TelUriParts::default()));
        assert_eq!(uri.number(), None);
        assert_eq!(uri.to_string(), "tel:");
    }
}
