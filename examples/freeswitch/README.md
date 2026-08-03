# FreeSWITCH dialplan integration

`fs-sip-uri` parses a SIP/tel/URN URI for an XML dialplan, replacing the
hand-written regexes that dialplans usually apply to header values.

FreeSWITCH runs it through the `spawn_stream` API, which uses `posix_spawnp`
with an argv split on blanks — **no shell**, so URI text arriving from a SIP
header never reaches a shell interpreter. `${spawn_stream(...)}` works anywhere
variables expand, including a `<condition>`'s `field` and `expression`.

## Build and install

The binary must run under whatever userspace FreeSWITCH itself runs under —
often a container image built elsewhere — so link it statically:

```sh
cargo build --profile release-min --target x86_64-unknown-linux-musl --example fs-sip-uri
install -Dm0755 target/x86_64-unknown-linux-musl/release-min/examples/fs-sip-uri \
        /etc/freeswitch/bin/fs-sip-uri
```

The `release-min` profile is size-tuned (`opt-level = "z"`, fat LTO, one codegen
unit, `panic = "abort"`, stripped), which roughly halves the static binary. A
plain `--release` build works too and is faster to produce.

Reference it as `$${conf_dir}/bin/fs-sip-uri` and the same dialplan works in
every deployment, since `conf_dir` resolves to the config tree wherever it is
mounted. FreeSWITCH has no macro facility for conditions, but a preprocessor
variable in `vars.xml` gets the path out of every call site:

```xml
<X-PRE-PROCESS cmd="set" data="fs_sip_uri=$${conf_dir}/bin/fs-sip-uri"/>
```

`$${fs_sip_uri}` is then substituted at XML parse time, so conditions read as
one call rather than a path. [dialplan-example.xml](dialplan-example.xml) is a
worked example using it: parse once, refuse to route on a parse failure, then
branch on the result.

## Reading one field

`get` prints the field to stdout and nothing else. `spawn_stream` captures that
stdout, so the binary's output *becomes* the `field` string, and `expression` is
the regex matched against it:

```xml
<condition field="${spawn_stream($${conf_dir}/bin/fs-sip-uri get user ${sip_h_X-Caller-Id-Number})}"
           expression="^(.+)$" break="on-true">
    <action application="set" data="caller_number=$1"/>
</condition>
```

`$1` is the first capture group of *this condition's* `expression`, so
`^(.+)$` means "matched something non-empty, and `$1` is all of it". `$2`, `$3`
and so on follow the remaining groups. They are only usable in that condition's
own actions.

If the expression has no capture group, `$1` is **not** substituted at all — the
action receives the literal two characters `$1`. It does not become empty and it
does not inherit a value from an earlier condition, so `set data="x=$1"` under a
group-less expression stores a literal `$1`. Always pair a `$1` with a `(...)`
in the same condition.

The three shapes that cover almost everything:

```xml
<!-- capture the value -->
<condition field="${spawn_stream(… get host ${uri})}" expression="^(.+)$" break="never">
    <action application="set" data="target_host=$1"/>
</condition>

<!-- branch on the component being absent; no capture needed -->
<condition field="${spawn_stream(… get user ${uri})}" expression="^$" break="on-true">
    <action application="log" data="ERR no user-part"/>
</condition>

<!-- branch on a specific value -->
<condition field="${spawn_stream(… get type ${uri})}" expression="^tel$" break="on-true">
    <action application="log" data="INFO tel: URI"/>
</condition>
```

A URI with no such component prints nothing and exits 0, so an absent component
and a parse failure both expand to the empty string and both match `^$` — see
the error contract below for telling them apart.

| field | value |
| --- | --- |
| `type` | `sip`, `tel`, `urn` or `other` |
| `scheme` | `sip`, `sips`, `tel`, `urn`, … |
| `uri` | the URI, re-serialized canonically |
| `user` | SIP user part, or the `tel:` number |
| `password` | SIP password |
| `host` | SIP host, IPv6 in brackets |
| `port` | explicit port only, never a scheme default |
| `nid` / `nss` | URN namespace id / namespace-specific string |
| `param.<name>` | URI parameter, after the host |
| `uparam.<name>` | user parameter, inside the userinfo before `@` |
| `header.<name>` | URI header, after `?` |

Parameter lookups are case-insensitive. A parameter present without a value
prints empty — FreeSWITCH cannot distinguish that from unset either way.

## Reading every field at once

Where the parse should be recorded rather than just tested — so that `info`,
`uuid_dump` and the CDR show what the parser actually saw — `vars` emits a
`multiset` payload:

```xml
<action application="multiset" inline="true"
        data="^^|${spawn_stream($${conf_dir}/bin/fs-sip-uri vars uri_ ${sip_h_X-Caller-Id-Number})}"/>
```

`sip:+15551234567;cpc=emergency@198.51.100.1;user=phone` then sets
`uri_type`, `uri_scheme`, `uri_user`, `uri_host`, `uri_uparam_cpc` and
`uri_param_user`. Absent components are not emitted, so they stay unset.

The `^^|` prefix tells `multiset` to split the payload on `|` instead of a
space. That delimiter cannot be assumed safe: `canonize_user` decodes `%3B` and
`%3D`, so a user part can legitimately contain `;` or `=`, and the parser
accepts a raw `|` in a user part as well. Rather than emit a payload that would
silently set an unintended variable, `vars` refuses:

```
fs-sip-uri: user contains '|', refusing to emit a payload
```

`multiset` splits each pair on its *first* `=`, so a value containing `=`
round-trips correctly.

### Clear the previous parse

Absent components are not emitted, so reusing a prefix would leave the earlier
parse's values in place for anything the new URI lacks — parsing
`sip:+15551234567@first.example.com` and then `sip:second.example.com` into the
same prefix would update `uri_host` while `uri_user` still held
`+15551234567`, a number belonging to the other URI. The same channel survives
`transfer` and `execute_extension`, so a sub-dialplan or a Lua script parsing a
second URI lands in the same namespace.

Each payload therefore ends with `<prefix>keys`, listing every variable it
sets, space-separated because that is `multiunset`'s default delimiter. Clear
before you set:

```xml
<action application="multiunset" data="${uri_keys}"/>
<action application="multiset"
        data="^^|${spawn_stream($${conf_dir}/bin/fs-sip-uri vars uri_ ${some_uri})}"/>
```

On the first parse `${uri_keys}` is empty and `multiunset` is a silent no-op,
so the pair is safe to use unconditionally. Separate processes cannot interfere
with each other; this flat namespace is the only shared thing here.

## Angle brackets

A bare `<sip:...>` wrapper is stripped. A display name or trailing header
parameters are header grammar rather than URI grammar and are rejected — split
the name-addr before calling this.

## Error contract

Failures print nothing on stdout, write one line to stderr, and exit non-zero:
1 for an unparsable URI or a payload `vars` refused to emit, 2 for a field name
that does not exist. FreeSWITCH logs the child's stderr and its exit status,
both naming the full command:

```
[WARNING] switch_core.c:3481 STDERR of cmd (…/fs-sip-uri get user garbage-not-a-uri):
          fs-sip-uri: cannot parse "garbage-not-a-uri": invalid URI: missing scheme
[WARNING] switch_core.c:3494 Exit status (256): …/fs-sip-uri get user garbage-not-a-uri
```

That status is the raw `waitpid` value rather than the exit code, so exit 1
logs as 256 and exit 2 as 512.

Nothing is logged on success. Because every failure mode expands to an empty
string, guard explicitly rather than letting an unparsed header fall through to
the next condition — an empty `${uri_user}` will otherwise build a URI out of
`${uri_host}` alone and reach the wrong endpoint:

```xml
<condition field="${uri_type}" expression="^$" break="on-true">
    <action application="log" data="ERR unparsable caller id header"/>
    <action application="hangup" data="NORMAL_UNSPECIFIED"/>
</condition>
```

`uri_type` is the field to test: `vars` always emits it on success, so its
absence means the parse failed rather than the URI lacking that component.
Where a single `get` is enough, match the failure instead of the success, so
the empty string lands in the reject branch:

```xml
<condition field="${spawn_stream($${fs_sip_uri} get type ${hdr})}"
           expression="^(?!tel$).*$" break="on-true">
    <action application="log" data="ERR expected a tel: URI"/>
    <action application="hangup" data="CALL_REJECTED"/>
</condition>
```

[dialplan-example.xml](dialplan-example.xml) puts both forms in context.

## Hang safety

`switch_stream_spawn` polls the child with an infinite timeout, so a child that
never exits wedges the session thread for the life of the call, during the
routing phase. This binary parses argv and exits: no network, no filesystem, no
retry loop. Keep it that way.

## Live test

`test-live.sh` builds the binary, installs it and `sip_uri_test.xml` into a
running FreeSWITCH's config tree, then routes a channel per case and reads the
parsed components back off the parked leg.

```sh
FS_CONF=~/fs FS_CLI="fs_cli -P 8022" ./test-live.sh
```

It needs `jq`, and it hangs up all channels between cases — point it at a test
instance, never production.
