//! Serde adapters that write a URI or host as its [`Display`] text and read
//! it back with the lenient parser.
//!
//! Use one with `#[serde(with = …)]`, or its `option` submodule for an
//! `Option` field. A read that fails reports the [`ParseError`](crate::ParseError), never the
//! text.
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! use sip_uri::{SipUri, Uri};
//!
//! #[derive(Serialize, Deserialize)]
//! struct Call {
//!     #[serde(with = "sip_uri::serde_str::uri")]
//!     to: Uri,
//!     #[serde(with = "sip_uri::serde_str::sip_uri::option", default)]
//!     contact: Option<SipUri>,
//! }
//!
//! let call: Call = serde_json::from_str(r#"{"to": "tel:+15551234567"}"#).unwrap();
//! assert_eq!(serde_json::to_string(&call).unwrap(), r#"{"to":"tel:+15551234567","contact":null}"#);
//! ```

use std::fmt::Display;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::UriParse;

fn serialize<T: Display, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

fn parse<T: UriParse, E: serde::de::Error>(text: &str) -> Result<T, E> {
    T::parse(text).map_err(E::custom)
}

fn deserialize<'de, T: UriParse, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
    parse(&String::deserialize(deserializer)?)
}

struct AsText<'a, T>(&'a T);

impl<T: Display> Serialize for AsText<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self.0)
    }
}

fn serialize_option<T: Display, S: Serializer>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(v) => serializer.serialize_some(&AsText(v)),
        None => serializer.serialize_none(),
    }
}

fn deserialize_option<'de, T: UriParse, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|text| parse(&text))
        .transpose()
}

macro_rules! adapter {
    ($($(#[$doc:meta])* $name:ident => $ty:ty;)*) => {$(
        $(#[$doc])*
        pub mod $name {
            use serde::{Deserializer, Serializer};

            /// Write the value as its `Display` text.
            pub fn serialize<S: Serializer>(value: &$ty, serializer: S) -> Result<S::Ok, S::Error> {
                super::serialize(value, serializer)
            }

            /// Read the value with the lenient parser.
            pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<$ty, D::Error> {
                super::deserialize(deserializer)
            }

            /// The same adapter for an `Option` field, `None` as null.
            pub mod option {
                use serde::{Deserializer, Serializer};

                /// Write the value as its `Display` text, or null.
                pub fn serialize<S: Serializer>(
                    value: &Option<$ty>,
                    serializer: S,
                ) -> Result<S::Ok, S::Error> {
                    super::super::serialize_option(value, serializer)
                }

                /// Read the value with the lenient parser, null as `None`.
                pub fn deserialize<'de, D: Deserializer<'de>>(
                    deserializer: D,
                ) -> Result<Option<$ty>, D::Error> {
                    super::super::deserialize_option(deserializer)
                }
            }
        }
    )*};
}

adapter! {
    /// [`Uri`](crate::Uri) as text.
    uri => crate::Uri;
    /// [`SipUri`](crate::SipUri) as text.
    sip_uri => crate::SipUri;
    /// [`TelUri`](crate::TelUri) as text.
    tel_uri => crate::TelUri;
    /// [`UrnUri`](crate::UrnUri) as text.
    urn_uri => crate::UrnUri;
    /// [`Host`](crate::Host) as text, IPv6 bracketed.
    host => crate::Host;
}
