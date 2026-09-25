# Design Rationale

## Non-conformant input parses and warns; only unusable input is an error

A URI this crate can represent and re-serialize is returned, whatever it breaks in the grammar, and each breach comes back as a typed warning beside the value. `Err` is reserved for input with no usable reading: no scheme, no host, a port that is not a number. A PSAP cannot refuse a call over a stray character in a caller's URI, and it owes the originating network a discrepancy report naming it, so rejecting loses the call and silent acceptance loses the report.

`parse_with_warnings` is the base; `FromStr` is the convenience that discards the warnings, and never accepts less than the base does. Tightening a check therefore means adding a warning, never a rejection.

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

## Display round-trips through the parser, not the input

`parse(display(parse(x))) == parse(x)` holds for every accepted input; `display(parse(x)) == x` does not, since canonization normalizes escapes and case. Any component the parser keeps must be emitted by Display, or the invariant breaks.

## The crate parses URIs, never header fields

Only `addr-spec` and `name-addr` are in scope. Header-level parameters after `>` belong to a SIP header parser, so `NameAddr` rejects trailing content rather than discarding it, and a test value carrying percent-encoded header params signals header grammar leaking into this layer.
