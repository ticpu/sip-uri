//! Zero-dependency SIP/SIPS, tel:, and URN parser.
//!
//! Implements RFC 3261 §19/§25 (SIP-URI, SIPS-URI), RFC 3966 (tel-URI),
//! and RFC 8141 (URN) with hand-written parsing and per-component
//! percent-encoding.
//!
//! # Examples
//!
//! ```
//! use sip_uri::{SipUri, TelUri, UrnUri, Uri, NameAddr};
//!
//! // Parse a SIP URI
//! let uri: SipUri = "sip:alice@example.com;transport=tcp".parse().unwrap();
//! assert_eq!(uri.user(), Some("alice"));
//! assert_eq!(uri.param("transport"), Some(&Some("tcp".to_string())));
//!
//! // Parse a tel: URI
//! let tel: TelUri = "tel:+15551234567;cpc=emergency".parse().unwrap();
//! assert_eq!(tel.number(), "+15551234567");
//! assert!(tel.is_global());
//!
//! // Parse a URN (e.g. NG911 service identifier)
//! let urn: UrnUri = "urn:service:sos".parse().unwrap();
//! assert_eq!(urn.nid(), "service");
//! assert_eq!(urn.nss(), "sos");
//!
//! // Parse a name-addr (display name + URI)
//! let na: NameAddr = r#""Alice" <sip:alice@example.com>"#.parse().unwrap();
//! assert_eq!(na.display_name(), Some("Alice"));
//!
//! // Dispatch on URI type
//! let uri: Uri = "urn:service:sos".parse().unwrap();
//! match uri {
//!     Uri::Sip(sip) => println!("SIP: {sip}"),
//!     Uri::Tel(tel) => println!("Tel: {tel}"),
//!     Uri::Urn(urn) => println!("URN: {urn}"),
//!     _ => println!("other"),
//! }
//! ```

mod error;
mod host;
mod name_addr;
pub(crate) mod params;
pub(crate) mod parse;
mod sip_uri;
mod tel_uri;
mod uri;
mod urn_uri;

pub use error::{
    ParseNameAddrError, ParseSipUriError, ParseTelUriError, ParseUriError, ParseUrnError,
};
pub use host::Host;
pub use name_addr::NameAddr;
pub use sip_uri::{Scheme, SipUri};
pub use tel_uri::TelUri;
pub use uri::Uri;
pub use urn_uri::UrnUri;
