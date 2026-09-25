## Project Type

Zero-dependency SIP/tel/URN URI parser library: RFC 3261 (SIP/SIPS), RFC 3966 (tel:), RFC 8141 (URN). Decisions and their reasons live in `docs/design-rationale.md`.

## No PII or Organization-Specific Data

**NEVER** include real phone numbers, real hostnames, organization names, internal URLs, or any other PII in source code, tests, or documentation. Use RFC-compliant test values only:

- Phone numbers: `+1555xxxxxxx` (555 prefix)
- IPv4: `198.51.100.x` or `203.0.113.x` (RFC 5737 TEST-NET)
- Domains: `example.com`, `example.org`, `example.net` (RFC 6761)
- IPv6: `2001:db8::x` (RFC 3849 documentation prefix)
- Organization names: "EXAMPLE CO", generic descriptions
- URN identifiers: synthetic hashes, `TEST` prefixes

The pre-commit hook runs gitleaks on staged content and refuses to commit without it.

## New RFC checks warn, never reject

A grammar check added to a parser pushes a `ParseWarning`; it never turns accepted input into an `Err`.

## `#[non_exhaustive]` on every public enum and public-field struct

Single-field error newtypes are exempt.

## No `assert!` / `unwrap()` in library code

## Scope Boundary

URIs only (`addr-spec`, `name-addr`), never SIP header field grammar. A test value with percent-encoded header-level params (`;tag=`, `;serviceurn=`) is header grammar leaking in. `NameAddr` is deprecated since 0.2.0 and must be removed in 0.3.0.

## Release Workflow

Use `/release` (`.claude/commands/release.md`); it owns the checks, changelog, tagging and publish steps.
