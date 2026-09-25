# Design Rationale

## Non-compliance warns; only empty input or another type's scheme is an error

Whatever an input breaks in the grammar, the parser returns what it could read and reports each breach as a typed warning beside the value. `Err` means only that there is nothing to read, or that a present scheme names a different kind of URI; `Uri` answers the latter with `Other`. A PSAP cannot refuse a call over a malformed caller URI, and it owes the originating network a discrepancy report naming the breach, so rejecting loses the call and silent acceptance loses the report.

`parse_with_warnings` is the base; `FromStr` discards the warnings, and `parse_strict` turns the first one into the error. All three run one path, so a check exists once, as a warning code, and tightening one never narrows what the lenient parsers accept.

## A spec-required component holds absence, and a warning reports it

A scheme, host, NID, NSS or tel number that is missing or unreadable is `None` with a warning, rather than the whole URI failing. A broken port or host costs that field, never the user part beside it.

## `Uri` without a scheme is `Other`, never a guessed type

Input without a scheme is kept as `Other` with a warning, even when it looks like `user@host`. Only the caller knows its context says SIP, and it re-parses as `SipUri` to get that reading.

## An unrecognized scheme is kept, lowercased

`Other` keeps the input as sent except for its scheme, lowercased because schemes compare case-insensitively, so `scheme()` reads the same for every variant and two spellings of one URI compare equal.

## A warning names the component and position, never the text

A warning carries which component broke, a typed code, the byte offset and whether the value survived, but no copy of the offending text. The user part is a caller number and warnings are what consumers log; the value itself is on the parsed struct for anyone entitled to it. Error messages follow the same rule.

## `@` discovery follows sofia-sip, and a header-shaped user part warns

The user part may carry `;` `?` `/` unescaped, so the delimiter is the first `@` after the first `@/;?#`, as in sofia-sip. Accepting a literal `@` in URI params and headers makes that ambiguous when there is no userinfo: `sip:host?From=a@b` reads as user `host?From=a` at host `b`. That is the only reading valid under the strict grammar, so the parse stands, and a userinfo containing `?` warns so the likely misparse is visible.

## User-params are split out of the user part

The `telephone-subscriber` form in userinfo is split into the user and its `;`-separated params, rather than kept flat as sofia-sip does, because NG9-1-1 carries `cpc` and `oli` there and consumers need them typed. A caller wanting the unsplit string reconstructs it from both accessors.

## `param-unreserved` accepts `@` and `,`, with a warning

Real SIP traffic and the sofia-sip torture corpus put both unescaped in URI params. They parse as literal param characters and are reported, since the grammar does not allow them.

## Header values are escaped canonically; user parts are kept as sent

A URI header value is re-encoded to its canonical form, every byte outside the `hnv` set escaped, so a literal and an escaped `@` compare equal and match what `encode_uri_header` produces. Header consumers decode before use, so nothing downstream reads the difference. A user part is compared against dialplan numbers as it stands and carries the unescaped `#` that phones send, so it is only normalized where escaping is optional, never re-encoded, and a character outside its set warns instead.

## Display is the only wire form; other renderings are adapters

Display emits the canonical form and is the one serializer, so `parse(display(parse(x))) == parse(x)` holds for every accepted input, while `display(parse(x)) == x` does not. Any component the parser keeps must be emitted by Display. Another rendering, such as a host without IPv6 brackets or a URI redacted for logs, is a method returning its own Display adapter, never a second serializer.

Display's output is API that no tooling guards: a trait impl cannot be deprecated and semver checks do not read output. A change to what it emits therefore ships only in a breaking release, after the rendering it replaces is available as an adapter.

Display carries the user part and password, so logs use the redacted rendering, never Display. Its default masks the whole userinfo, or a tel: number, and the caller relaxes it: what a log may carry is the deployment's policy, and only a mask that starts closed fails safe.

## Builder input is canonized like parsed input

A value handed to a builder is escaped for its component the way parsing canonizes one, so a caller-supplied string cannot inject params or headers, and a built URI obeys the same round-trip rule as a parsed one.

## The crate parses URIs, never header fields

Only `addr-spec` is in scope; display names and header-level parameters belong to a SIP header parser. Text still wrapped in `<>` parses as `Other` with a scheme warning, and a test value carrying percent-encoded header params signals header grammar leaking into this layer.
