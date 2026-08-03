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
mounted.

## Reading one field

```xml
<condition field="${spawn_stream($${conf_dir}/bin/fs-sip-uri get user ${sip_h_X-Caller-Id-Number})}"
           expression="^(.+)$" break="on-true">
    <action application="set" data="caller_number=$1"/>
</condition>
```

`get` prints the field and nothing else. A URI with no such component prints
nothing and exits 0, so an absent component and a parse failure both expand to
the empty string — see the error contract below for telling them apart.

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
string, guard the dialplan explicitly rather than letting an unparsed header
fall through to the next condition:

```xml
<condition field="${uri_host}" expression="^$" break="on-true">
    <action application="log" data="ERR unparsable caller id header"/>
    <action application="hangup" data="NORMAL_UNSPECIFIED"/>
</condition>
```

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
