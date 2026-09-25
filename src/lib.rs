//! Zero-dependency SIP/SIPS, tel:, and URN parser.
//!
//! Implements RFC 3261 §19/§25 (SIP-URI, SIPS-URI), RFC 3966 (tel-URI),
//! and RFC 8141 (URN) with hand-written parsing and per-component
//! percent-encoding.
//!
//! # Examples
//!
//! ```
//! use sip_uri::{SipUri, TelUri, UriParse, UrnUri, Uri};
//!
//! // Parse a SIP URI
//! let uri = SipUri::parse("sip:alice@example.com;transport=tcp").unwrap();
//! assert_eq!(uri.user(), Some("alice"));
//! assert_eq!(uri.param("transport"), Some(Some("tcp")));
//!
//! // Parse a tel: URI
//! let tel = TelUri::parse("tel:+15551234567;cpc=emergency").unwrap();
//! assert_eq!(tel.number(), Some("+15551234567"));
//! assert!(tel.is_global());
//!
//! // Parse a URN (e.g. NG911 service identifier)
//! let urn = UrnUri::parse("urn:service:sos").unwrap();
//! assert_eq!(urn.nid(), Some("service"));
//! assert_eq!(urn.nss(), Some("sos"));
//!
//! // Dispatch on URI type
//! let uri = Uri::parse("urn:service:sos").unwrap();
//! match uri {
//!     Uri::Sip(sip) => println!("SIP: {sip}"),
//!     Uri::Tel(tel) => println!("Tel: {tel}"),
//!     Uri::Urn(urn) => println!("URN: {urn}"),
//!     Uri::Other(raw) => println!("other: {raw}"),
//!     _ => {}
//! }
//! ```

mod canon;
mod error;
mod grammar;
mod host;
mod params;
mod parser;
mod redact;
mod sip_uri;
mod tel_uri;
mod uri;
mod urn_uri;
mod warning;

pub use canon::{decode_user, encode_uri_header};
pub use error::ParseError;
pub use host::{Bare, Host, Hostname};
pub use parser::UriParse;
pub use redact::{Redacted, Redaction, UriRedact, UserMask};
pub use sip_uri::{Scheme, SipUri, SipUriParts};
pub use tel_uri::{TelUri, TelUriParts};
pub use uri::{OtherUri, Uri};
pub use urn_uri::{UrnUri, UrnUriParts};
pub use warning::{Component, ParseWarning, Parsed, WarningCode, WarningKind};
