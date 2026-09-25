# sip-uri-types

SIP/SIPS (RFC 3261), tel: (RFC 3966) and URN (RFC 8141) URI value types for Rust.

Each component is held in one canonical form, whether the value was built or parsed, so equality, hashing and `Display` do not depend on how the URI was spelled. Crates that expose a URI in their public API name these types, which change only when a value's identity does.

Parsing, warnings and log redaction live in [sip-uri](https://crates.io/crates/sip-uri), which parses into these types and re-exports them.

## License

MIT OR Apache-2.0 — see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
