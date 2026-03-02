## Project Type

Zero-dependency SIP/tel/URN URI parser library. RFC 3261 (SIP/SIPS),
RFC 3966 (tel:), and RFC 8141 (URN).

## No PII or Organization-Specific Data

**NEVER** include real phone numbers, real hostnames, organization names,
internal URLs, or any other PII in source code, tests, or documentation.
Use RFC-compliant test values only:

- Phone numbers: `+1555xxxxxxx` (555 prefix)
- IPv4: `198.51.100.x` or `203.0.113.x` (RFC 5737 TEST-NET)
- Domains: `example.com`, `example.org`, `example.net` (RFC 6761)
- IPv6: `2001:db8::x` (RFC 3849 documentation prefix)
- Organization names: "EXAMPLE CO", generic descriptions
- URN identifiers: synthetic hashes, `TEST` prefixes

The pre-commit hook runs gitleaks to enforce this.

## Build & Test

```sh
cargo fmt --all
cargo check --message-format=short
cargo clippy --fix --allow-dirty --message-format=short
cargo test
```

## `#[non_exhaustive]` Policy

Same as freeswitch-esl-tokio: all public enums and public-field structs get
`#[non_exhaustive]`. Single-field error newtypes are exempt.

## Key Design Decisions

- Hand-written parser, no regex or nom -- the grammar is regular, deps are zero
- Per-component percent-encoding per RFC 3261 §25 ABNF
- SIP user-part allows `;/?/` unescaped (user-unreserved)
- User-params (`;` within userinfo before `@`) split out from user -- this is the
  correct parse of the `telephone-subscriber` production in userinfo, not a feature
  gate decision. Users who want the raw unsplit string reconstruct from `user()` +
  `user_params()`. Sofia-sip's flat approach is a C-level simplification.
- `@` discovery: sofia-sip two-phase algorithm -- scan to first `@/;?#`, then scan
  forward for `@`. Handles `?` and `/` in user-part correctly.
- Password split on first `:` within userinfo (`:` is NOT in user-unreserved)
- Param name comparison is case-insensitive per RFC 3261 §19.1.4
- Host names are lowercased on parse
- Scheme stored and compared case-insensitively (`SIP:` accepted, stored as `sip`)
- `param-unreserved` extended with `@` and `,` for real-world SIP compatibility
  (sofia-sip torture tests include these in URI params)
- Display round-trip: `parse(display(x)) == x` for canonical forms. Note:
  canonization is lossy (percent-encoding normalization), so
  `display(parse(raw)) != raw` but `parse(display(parse(raw))) == parse(raw)`
- No `assert!/unwrap()` in library code -- same correctness-over-recovery policy
  as freeswitch-esl-tokio

## Character Classes (RFC 3261 §25)

- `unreserved = ALPHA / DIGIT / mark`
- `mark = "-" / "_" / "." / "!" / "~" / "*" / "'" / "(" / ")"`
- `user-unreserved = "&" / "=" / "+" / "$" / "," / ";" / "?" / "/"`
- `param-unreserved = "[" / "]" / "/" / ":" / "&" / "+" / "$"`
- `hnv-unreserved = "[" / "]" / "/" / "?" / ":" / "+" / "$"`
