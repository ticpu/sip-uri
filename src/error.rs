use std::fmt;

use crate::warning::ParseWarning;

/// Error returned by every parser in this crate.
///
/// Lenient parsing fails only on [`Empty`](ParseError::Empty) and
/// [`SchemeMismatch`](ParseError::SchemeMismatch); strict parsing also
/// returns the first grammar breach as
/// [`NonConformant`](ParseError::NonConformant). Display never quotes the
/// input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// The input is empty, or holds nothing this type can represent.
    Empty,
    /// The input's scheme names another kind of URI.
    SchemeMismatch,
    /// A strict parse met a grammar breach.
    NonConformant(ParseWarning),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => f.write_str("empty input"),
            ParseError::SchemeMismatch => f.write_str("scheme belongs to another URI type"),
            ParseError::NonConformant(w) => write!(f, "non-conformant URI: {w}"),
        }
    }
}

impl std::error::Error for ParseError {}
