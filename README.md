# sip-uri

Zero-dependency SIP/SIPS, tel:, and URN parser for Rust.

Implements RFC 3261 (SIP-URI, SIPS-URI), RFC 3966 (tel-URI), and
RFC 8141 (URN) with hand-written parsing and per-component
percent-encoding.

```rust
use sip_uri::{SipUri, TelUri, UrnUri, Uri};

let uri: SipUri = "sip:alice@example.com;transport=tcp".parse().unwrap();
assert_eq!(uri.user(), Some("alice"));
assert_eq!(uri.host().to_string(), "example.com");
assert_eq!(uri.param("transport"), Some(&Some("tcp".to_string())));

let tel: TelUri = "tel:+15551234567;cpc=emergency".parse().unwrap();
assert_eq!(tel.number(), "+15551234567");
assert!(tel.is_global());

let urn: UrnUri = "urn:service:sos".parse().unwrap();
assert_eq!(urn.nid(), "service");
assert_eq!(urn.nss(), "sos");
```

```toml
[dependencies]
sip-uri = "0.2"
```

## Types

| Type | Description |
|---|---|
| `SipUri` | SIP or SIPS URI with user, host, port, params, headers, fragment |
| `TelUri` | tel: URI with number, params, fragment |
| `UrnUri` | URN with NID, NSS, and optional r/q/f components |
| `Uri` | Enum dispatching `Sip` / `Tel` / `Urn` / `Other` based on scheme |
| `NameAddr` | *Deprecated* -- display name + URI (`"Alice" <sip:...>`), will be removed in 0.3.0 |
| `Host` | IPv4, IPv6, or hostname |
| `Scheme` | `Sip` or `Sips` |

All types implement `FromStr`, `Display`, `Debug`, `Clone`, `PartialEq`, and `Eq`.
Parsing is case-insensitive for schemes, hosts, and parameter names.
`Display` output round-trips through `FromStr`.

## SipUri

```rust
use sip_uri::{SipUri, Scheme};

// Full SIP URI with user-params, password, port, params, headers
let uri: SipUri = "sips:+15551234567;cpc=emergency:secret@[2001:db8::1]:5061;user=phone?Subject=test"
    .parse().unwrap();

assert_eq!(uri.scheme(), Scheme::Sips);
assert_eq!(uri.user(), Some("+15551234567"));
assert_eq!(uri.user_params(), &[("cpc".into(), Some("emergency".into()))]);
assert_eq!(uri.password(), Some("secret"));
assert_eq!(uri.port(), Some(5061));
assert_eq!(uri.param("user"), Some(&Some("phone".into())));
assert_eq!(uri.header("Subject"), Some("test"));
```

### User-params

SIP URIs with `user=phone` follow the telephone-subscriber production from
RFC 3966. Parameters within the userinfo (before `@`) are split from the
user part and exposed via `user_params()`:

```rust
use sip_uri::SipUri;

// NG911 pattern: user-params carry tel: semantics inside a SIP URI
let uri: SipUri = "sip:+15551234567;cpc=emergency;oli=0@198.51.100.1;user=phone"
    .parse().unwrap();

assert_eq!(uri.user(), Some("+15551234567"));
assert_eq!(uri.user_params().len(), 2);
```

### Builder

```rust
use sip_uri::{SipUri, Scheme, Host};
use std::net::Ipv4Addr;

let uri = SipUri::new(Host::IPv4(Ipv4Addr::new(198, 51, 100, 1)))
    .with_scheme(Scheme::Sips)
    .with_user("+15551234567")
    .with_port(5061)
    .with_param("transport", Some("tcp".into()));

assert_eq!(uri.to_string(), "sips:+15551234567@198.51.100.1:5061;transport=tcp");
```

## TelUri

```rust
use sip_uri::TelUri;

let uri: TelUri = "tel:+15551234567;cpc=emergency;oli=0".parse().unwrap();
assert_eq!(uri.number(), "+15551234567");
assert!(uri.is_global());
assert_eq!(uri.param("cpc"), Some(&Some("emergency".into())));

// Local numbers (no + prefix)
let local: TelUri = "tel:911".parse().unwrap();
assert!(!local.is_global());
```

## UrnUri

URN parsing per RFC 8141. Used in SIP for NG911 service identifiers,
3GPP IMS service types, GSMA IMEI, and NENA call/incident tracking.

```rust
use sip_uri::UrnUri;

// NG911 emergency service identifier
let urn: UrnUri = "urn:service:sos.fire".parse().unwrap();
assert_eq!(urn.nid(), "service");
assert_eq!(urn.nss(), "sos.fire");

// NENA call tracking identifier
let urn: UrnUri = "urn:nena:callid:abc123:host.example.com".parse().unwrap();
assert_eq!(urn.nid(), "nena");
assert_eq!(urn.nss(), "callid:abc123:host.example.com");

// 3GPP IMS service
let urn: UrnUri = "urn:urn-7:3gpp-service.ims.icsi.mmtel".parse().unwrap();
assert_eq!(urn.nid(), "urn-7");

// Optional RFC 8141 components (resolution, query, fragment)
let urn: UrnUri = "urn:example:resource?+resolve?=query#section".parse().unwrap();
assert_eq!(urn.r_component(), Some("resolve"));
assert_eq!(urn.q_component(), Some("query"));
assert_eq!(urn.f_component(), Some("section"));
assert_eq!(urn.assigned_name(), "urn:example:resource");
```

NID is validated per RFC 8141 (2-32 chars, alphanum bookends) and stored
lowercase. NSS percent-encoding hex digits are uppercased for canonical
comparison but never decoded.

## NameAddr (deprecated)

`NameAddr` is deprecated since 0.2.0 and will be removed in 0.3.0.
`name-addr` is SIP header-level grammar, not URI-level. Header-level
parameter parsing (`;tag=`, `;expires=`, `;serviceurn=`, etc.) belongs in
a SIP header parser such as
[freeswitch-types](https://crates.io/crates/freeswitch-types).

## Percent-encoding

Per-component percent-encoding follows RFC 3261 rules:

- Unreserved characters are decoded (`%41` -> `A`)
- Reserved characters stay encoded (`%40` stays `%40` in user-part)
- Hex digits are normalized to uppercase (`%3d` -> `%3D`)
- Each URI component has its own allowed character set

```rust
use sip_uri::SipUri;

// Percent-encoded quotes in user-part are preserved
let uri: SipUri = r#"sip:%22foo%22@example.com"#.parse().unwrap();
assert_eq!(uri.user(), Some(r#"%22foo%22"#));
```

## Design

- **Zero dependencies** -- not even `percent-encoding`. The subset needed is
  trivial and avoids transitive dep churn.
- **Hand-written parser** -- the SIP URI grammar is regular enough that nom/regex
  are unnecessary overhead. Parsing follows the sofia-sip two-phase `@` discovery
  algorithm for correct handling of reserved characters in user-parts.
- **Case-insensitive where required** -- scheme and parameter name lookup are
  case-insensitive per RFC. Host names are lowercased.
- **`#[non_exhaustive]`** -- on `Uri`, `Host`, and `Scheme` enums for
  forward-compatible matching.
- **Fragment support** -- `SipUri` and `TelUri` parse and round-trip `#fragment`
  components (accepted permissively, matching sofia-sip behavior).
- **Any-scheme fallback** -- `Uri::Other` stores unrecognized schemes (http,
  https, data, etc.) as raw strings, so `NameAddr` can parse SIP headers like
  `Call-Info` that carry non-SIP URIs.

## RFC coverage

- **RFC 3261 19/25** -- SIP-URI, SIPS-URI syntax, comparison, percent-encoding
- **RFC 3966** -- tel-URI (global/local numbers, visual separators, parameters)
- **RFC 8141** -- URN syntax (NID, NSS, r/q/f components)

## Development

```sh
cargo fmt --all
cargo clippy --message-format=short
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --no-deps
cargo test
```

## Other Rust SIP URI crates

- [rsip](https://crates.io/crates/rsip) -- full SIP library with heavy deps
  (nom, bytes, md5, sha2, uuid). No tel: URI support, no user-param extraction.
- [rvoip-sip-core](https://crates.io/crates/rvoip-sip-core) -- alpha with a
  massive dependency tree.

Neither is a focused, zero-dep URI-only parser.

## License

MIT OR Apache-2.0 -- see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
