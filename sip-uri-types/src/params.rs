use std::fmt;

use crate::canon;

type Pair = (String, Option<String>);

type Canonize = fn(&str, Option<&str>) -> Pair;

/// Iterator over the `(name, value)` pairs of a [`Params`], [`UserParams`] or
/// [`Headers`], in order.
#[derive(Debug, Clone)]
pub struct Pairs<'a>(std::slice::Iter<'a, Pair>);

impl<'a> Iterator for Pairs<'a> {
    type Item = (&'a str, Option<&'a str>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|(name, value)| (name.as_str(), value.as_deref()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0
            .size_hint()
    }
}

impl DoubleEndedIterator for Pairs<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|(name, value)| (name.as_str(), value.as_deref()))
    }
}

impl ExactSizeIterator for Pairs<'_> {}

/// Push the canonized pair, unless it is a valueless empty name: its bare
/// delimiter parses as nothing.
fn push_pair(pairs: &mut Vec<Pair>, name: &str, value: Option<&str>, canonize: Canonize) {
    if !name.is_empty() || value.is_some() {
        pairs.push(canonize(name, value));
    }
}

macro_rules! pair_list {
    ($(#[$doc:meta])* $name:ident, $canonize:path) => {
        $(#[$doc])*
        ///
        /// Names and values are held in canonical form: [`push`](Self::push),
        /// [`From`] and [`FromIterator`] canonize what they are given, and a
        /// pair with an empty name and no value is dropped. Order and
        /// duplicates are kept.
        ///
        /// With the `serde` feature it serializes as a sequence of
        /// `[name, value]` pairs, the value `null` when absent, and
        /// deserializes through the same canonization.
        #[derive(Clone, Default, PartialEq, Eq, Hash)]
        pub struct $name(Vec<Pair>);

        impl $name {
            /// An empty list.
            pub fn new() -> Self {
                Self::default()
            }

            /// Number of pairs.
            pub fn len(&self) -> usize {
                self.0
                    .len()
            }

            /// Whether there are no pairs.
            pub fn is_empty(&self) -> bool {
                self.0
                    .is_empty()
            }

            /// The `(name, value)` pairs, in order.
            pub fn iter(&self) -> Pairs<'_> {
                Pairs(
                    self.0
                        .iter(),
                )
            }

            /// The value of the first pair named `name`, compared
            /// case-insensitively: `Some(None)` when it has no value.
            pub fn get(&self, name: &str) -> Option<Option<&str>> {
                self.iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v)
            }

            /// Append a pair, canonized.
            pub fn push(&mut self, name: impl AsRef<str>, value: Option<&str>) {
                push_pair(&mut self.0, name.as_ref(), value, $canonize);
            }

            /// Append a pair, canonized, and return the list.
            pub fn with(mut self, name: impl AsRef<str>, value: Option<&str>) -> Self {
                self.push(name, value);
                self
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list()
                    .entries(self.iter())
                    .finish()
            }
        }

        impl<'a> IntoIterator for &'a $name {
            type Item = (&'a str, Option<&'a str>);
            type IntoIter = Pairs<'a>;

            fn into_iter(self) -> Pairs<'a> {
                self.iter()
            }
        }

        impl<N: AsRef<str>, V: AsRef<str>> FromIterator<(N, Option<V>)> for $name {
            fn from_iter<I: IntoIterator<Item = (N, Option<V>)>>(pairs: I) -> Self {
                let mut list = Self::default();
                for (name, value) in pairs {
                    list.push(
                        name,
                        value
                            .as_ref()
                            .map(AsRef::as_ref),
                    );
                }
                list
            }
        }

        impl From<Vec<(String, Option<String>)>> for $name {
            fn from(pairs: Vec<(String, Option<String>)>) -> Self {
                pairs
                    .into_iter()
                    .collect()
            }
        }
    };
}

pair_list!(
    /// Parameters of a SIP URI after the host, or of a tel: URI.
    Params,
    canon::canonize_param
);

pair_list!(
    /// Parameters inside a SIP URI's userinfo, after the user part.
    UserParams,
    canon::canonize_user_param
);

pair_list!(
    /// Headers of a SIP URI, after `?`. A header written without `=` has no
    /// value, and one written `name=` has an empty one.
    Headers,
    canon::canonize_header
);

/// Write each pair as `;name` or `;name=value`.
pub(crate) fn format_params(pairs: Pairs<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (name, value) in pairs {
        write!(f, ";{name}")?;
        if let Some(v) = value {
            write!(f, "={v}")?;
        }
    }
    Ok(())
}

/// Write headers as `?name=value&name`, a header without a value as its name.
pub(crate) fn format_headers(headers: &Headers, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (i, (name, value)) in headers
        .iter()
        .enumerate()
    {
        let sep = if i == 0 { '?' } else { '&' };
        write!(f, "{sep}{name}")?;
        if let Some(v) = value {
            write!(f, "={v}")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_is_case_insensitive_and_first_match() {
        let params = Params::new()
            .with("Transport", Some("tcp"))
            .with("user", Some("phone"))
            .with("transport", Some("udp"))
            .with("lr", None);
        assert_eq!(params.get("transport"), Some(Some("tcp")));
        assert_eq!(params.get("USER"), Some(Some("phone")));
        assert_eq!(params.get("lr"), Some(None));
        assert_eq!(params.get("missing"), None);
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn every_constructor_canonizes() {
        let pushed = Params::new().with("a;b", Some("%41@"));
        let collected: Params = [("a;b", Some("%41@"))]
            .into_iter()
            .collect();
        let converted = Params::from(vec![("a;b".to_string(), Some("%41@".to_string()))]);
        assert_eq!(pushed, collected);
        assert_eq!(pushed, converted);
        assert_eq!(
            pushed
                .iter()
                .collect::<Vec<_>>(),
            [("a%3Bb", Some("A%40"))]
        );
    }

    #[test]
    fn kinds_canonize_by_their_own_grammar() {
        let user = UserParams::new().with("n", Some("a=b"));
        let uri = Params::new().with("n", Some("a=b"));
        assert_eq!(user.get("n"), Some(Some("a=b")));
        assert_eq!(uri.get("n"), Some(Some("a%3Db")));
    }

    #[test]
    fn empty_name_without_value_is_dropped() {
        let headers = Headers::new()
            .with("", None)
            .with("", Some(""))
            .with("h", None);
        assert_eq!(
            headers
                .iter()
                .collect::<Vec<_>>(),
            [("", Some("")), ("h", None)]
        );
    }
}
