# Migrating from sip-uri 0.2 to 0.3

sip-uri 0.3 changes three things about the crate:

- Parsing rarely fails. A non-conformant URI parses, and each breach of the grammar comes back as a typed warning.
- Every URI component has one canonical form, whether the value was parsed or built.
- The value types moved to their own crate, sip-uri-types, which aims at 1.x.

Most of the API changes on this page follow from one of those three.

## Crates: sip-uri-types and sip-uri

The URI types (`Uri`, `SipUri`, `TelUri`, `UrnUri`, `OtherUri`, `Host`, `Hostname`, `Scheme`) are now defined in [sip-uri-types](https://crates.io/crates/sip-uri-types). sip-uri parses into them and re-exports them, so `use sip_uri::SipUri` keeps working.

Which to depend on:

- An application that parses URIs depends on `sip-uri`.
- A library that only holds or passes URIs, and names them in its public API, depends on `sip-uri-types`.

sip-uri-types changes only when a value's identity changes: its components, its canonical form, equality, or Display. sip-uri's parse policy moves on every minor release, and a library that exposes `sip_uri_types::Uri` does not break when it does. Before 0.3, every sip-uri minor was also a breaking release for every crate that exposed a URI.

## Parsing is the `UriParse` trait, not `FromStr`

```rust
// 0.2
let uri: Uri = s.parse()?;
let sip: SipUri = s.parse()?;

// 0.3
use sip_uri::UriParse;
let uri = Uri::parse(s)?;
let sip = SipUri::parse(s)?;
```

Rust's orphan rule lets `FromStr` be implemented only in the crate that defines the type, or in the crate that defines the trait. The types live in sip-uri-types and the parser lives in sip-uri, so the parser is offered as an extension trait. `UriParse` has `parse`, `parse_with_warnings` and `parse_strict`, and it is implemented for `Uri`, `SipUri`, `TelUri`, `UrnUri` and `Host`. It also works as a function value, for example `<Uri as UriParse>::parse` as a clap `value_parser`.

## Non-conformant input parses, with warnings

In 0.2, most grammar breaches were an `Err`. In 0.3, the parser returns whatever it could read and reports each breach:

```rust
use sip_uri::{Component, SipUri, UriParse, WarningCode};

let parsed = SipUri::parse_with_warnings("sip:+15551234567@example.com:+5060").unwrap();
assert_eq!(parsed.value.port(), Some(5060));
assert_eq!(parsed.warnings[0].component, Component::Port);
assert_eq!(parsed.warnings[0].code, WarningCode::SignedPort);
```

This matters wherever the application can't just drop the input. A server handling a call usually still has to handle it when the caller's URI is malformed, and what gets the sending side fixed is a report naming the breach. Rejecting loses the call; accepting silently loses the report.

A missing or unreadable component costs only that component. The accessors for components the grammar requires became `Option`:

| 0.2 | 0.3 |
|---|---|
| `SipUri::scheme() -> Scheme` | `Option<Scheme>` |
| `SipUri::host() -> &Host` | `Option<&Host>` |
| `TelUri::number() -> &str` | `Option<&str>` |
| `UrnUri::nid()`, `nss() -> &str` | `Option<&str>`, so write `urn.nid() == Some("service")` |
| `Uri::scheme() -> &str` | `Option<&str>`, lowercase for every variant |

A few inputs that were errors now parse to something:

- **Missing scheme:** `"joe@example.com"` as `SipUri` has scheme `None` and a `MissingScheme` warning. As `Uri`, it is `Uri::Other`, since `Uri` never guesses a type for scheme-less text. Parse it as `SipUri` when your context says it is SIP.
- **Wildcard:** `"*"` as `Uri` is `Uri::Other` with a `Wildcard` warning.
- **Brackets:** `"<sip:alice@example.com>"` as `Uri` is `Uri::Other` with a scheme warning. That text is header grammar; see [Display names](#display-names-and-header-params) below.

### Refusing non-conformant input

`parse` accepts exactly what `parse_with_warnings` accepts, and drops the warnings. So code that used a failed parse to refuse bad input has to ask explicitly:

```rust
// 0.2: a failed parse meant "malformed"
let Ok(uri) = s.parse::<SipUri>() else { return reject() };

// 0.3: parse_strict turns the first warning into the error
let Ok(uri) = SipUri::parse_strict(s) else { return reject() };

// 0.3: or look at the warnings and decide
let parsed = SipUri::parse_with_warnings(s)?;
if parsed.has_warnings() {
    log_discrepancy(&parsed.warnings);
}
```

`Parsed::into_strict()` does the same after the fact. The strict parser runs the lenient one and checks its warnings, so both accept and read input the same way; they differ only in what they return.

## One `ParseError` replaces the five error types

| 0.2 | 0.3 |
|---|---|
| `ParseSipUriError`, `ParseTelUriError`, `ParseUrnError`, `ParseHostError`, `ParseUriError`, `ParseNameAddrError` | `ParseError` |

At first sight this looks like a loss of detail. It isn't. Each 0.2 type was a newtype over a `String`: a message, often quoting the input, and nothing a program could match on. What set them apart was the type they came from, not anything about what went wrong.

In 0.3, nearly everything those messages described is a typed `ParseWarning`. Each warning says:

- which component broke (`Component`);
- what broke (`WarningCode`);
- the byte offset (`position`);
- whether the value survived (`WarningKind::Recovered`) or the component was dropped (`WarningKind::Lost`).

That leaves an error only two cases, and both are the same for every URI type:

```rust
#[non_exhaustive]
pub enum ParseError {
    /// Nothing to read.
    Empty,
    /// The scheme names another URI type, e.g. `tel:` parsed as `SipUri`.
    SchemeMismatch,
    /// A strict parse met a grammar breach; the warning says which.
    NonConformant(ParseWarning),
}
```

One type also means `?` works across `Uri`, `SipUri`, `TelUri`, `UrnUri` and `Host` with no `From` impls, and a caller can match on the cause, including the full warning under `parse_strict`.

Neither errors nor warnings quote the input. A user part often holds a phone number, and errors and warnings are what ends up in logs. The value itself is on the parsed struct for code entitled to read it.

## One canonical form per component

Every component holds its conformant characters literal, decodes escapes of unreserved characters only, and holds every other byte as an uppercase `%XX`. Builders, parts constructors, deserialization and the parser all go through the same canonizer. A user part also keeps a literal `#`, which phones send unescaped; `%23` stays a distinct value.

What changes for existing code:

| Input | 0.2 | 0.3 |
|---|---|---|
| user `a b` (a raw space) | kept raw | `a%20b`, with a warning |
| param `maddr=a@b` | kept raw | `a%40b`, with a warning |
| `%3B` in a user part | decoded to `;` | kept as `%3B`, since `;` there would start a user-param |
| `%2B` in a user part | decoded to `+` | kept as `%2B`: RFC 3261 counts an escaped reserved character distinct from the literal |
| `%61` in a user part | decoded to `a` | decoded to `a`, as every escape of an unreserved character is |
| param `x=a,b` | kept raw | `a%2Cb`, with a warning |
| `with_user("x;cpc=1@evil")` | written verbatim | escaped, so it can't add params or change the host |
| `TelUri::new("+1555;x=1")` | Display re-parses with a param `x` | `+1555%3Bx=1` |
| `Host::Hostname("EXAMPLE.COM".into())` | kept as given | `example.com`, equal to the parsed host |
| `Host::Hostname("198.51.100.1".into())` in a URI | kept as a hostname | held as `Host::IPv4`, equal to the parsed host |
| `Uri::Other` text with a space, CRLF or non-ASCII | kept raw | escaped as `%20`, `%0D%0A`, one `%XX` per byte; nothing is decoded |

Two consequences:

- Two spellings of one value compare equal, so `a b` and `a%20b` are the same URI.
- Display never prints a byte that changes how the URI parses, or that would break a header line. Caller-supplied text can't inject params, headers or a host.

Conformant input is unchanged, including phone numbers with `*`, `#` and `+`. `decode_user` still returns the logical bytes of a user part when you need them.

Builders and parts structs take URI text, not logical values: `with_user("%2B1")` holds `%2B1`, distinct from `+1`. Code that fed a builder already-decoded text keeps working as long as that text holds no `%`.

## Constructing values

| 0.2 | 0.3 |
|---|---|
| builders only | builders, plus `SipUriParts`, `TelUriParts`, `UrnUriParts` through `From`, and `into_parts()` |
| `Uri::Other(String)` | `Uri::Other(OtherUri)`, built with `OtherUri::new(scheme, rest)`; `as_other()` and `into_other()` are unchanged |
| `Host::Hostname(String)` | `Host::Hostname(Hostname)`; `"x".into()` still builds one, lowercased |

The parts structs are `#[non_exhaustive]`, so start from `Default` and assign fields:

```rust
use sip_uri::{Host, Scheme, SipUri, SipUriParts};

let mut parts = SipUriParts::default();
parts.scheme = Some(Scheme::Sip);
parts.user = Some("alice".into());
parts.host = Some(Host::Hostname("example.com".into()));
let uri = SipUri::from(parts);
assert_eq!(uri.to_string(), "sip:alice@example.com");
```

Every field starts absent, the scheme included: without `parts.scheme` the URI prints as `alice@example.com`.

A constructor given an empty component stores it as absent: `with_user("")` gives a URI with no user.

## Params

`param()` on `SipUri` and `TelUri` returns `Option<Option<&str>>`, which tells a missing param from one present without a value:

```rust
use sip_uri::{SipUri, UriParse};

let uri = SipUri::parse("sip:example.com;transport=tcp;lr").unwrap();
assert_eq!(uri.param("transport"), Some(Some("tcp")));  // ;transport=tcp
assert_eq!(uri.param("lr"), Some(None));                // ;lr
assert_eq!(uri.param("maddr"), None);                   // absent
```

The new `user_param()` looks up the params inside the userinfo in the same way.

## Logging

`Display` writes the user part and password. For logs, use the redacted rendering, which masks the whole userinfo (or a tel: number) by default:

```rust
use sip_uri::{Redaction, SipUri, UriParse, UriRedact, UserMask};

let uri = SipUri::parse("sip:+15551234567:pw@example.com").unwrap();
assert_eq!(uri.redacted(Redaction::default()).to_string(), "sip:***@example.com");
let keep4 = Redaction::default().user(UserMask::KeepLast(4));
assert_eq!(uri.redacted(keep4).to_string(), "sip:+xxxxxxx4567:***@example.com");
```

`redacted()` comes from the `UriRedact` trait, so import it.

## Serde

A new `serde` feature serializes a value as its parts and deserializes it through the same canonizing constructor. To read and write a URI as text instead, use the adapters in `sip_uri::serde_str` with `#[serde(with = …)]`. The [README](../README.md#serde) shows both forms.

## Removed

| 0.2 | 0.3 |
|---|---|
| `NameAddr` | `sip_header::SipHeaderAddr`, or `Uri` for a bare URI |
| `Host::fmt_uri(f)` | `Display` |

## Display names and header params

sip-uri parses URIs only (`addr-spec`). `"Alice" <sip:alice@example.com>;tag=abc` is SIP header grammar: parse it with [`SipHeaderAddr`](https://docs.rs/sip-header/latest/sip_header/struct.SipHeaderAddr.html) from [sip-header](https://crates.io/crates/sip-header). Code written against 0.2's `NameAddr` moves there.

## Also new

- `SipUri`, `TelUri`, `UrnUri` and `Uri` implement `Hash`, consistent with `Eq`. Both compare the canonical form component by component, never RFC 3261 §19.1.4 equivalence: param order, param and header name case, tel: visual separators and a hostname's trailing dot all count.
