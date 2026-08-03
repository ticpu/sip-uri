#!/bin/bash
# Exercise fs-sip-uri against a running FreeSWITCH: build it, install it into
# the conf tree, then route a channel through sip_uri_test.xml per case and
# read the parsed components back off the parked leg.
#
#   FS_CONF=~/fs FS_CLI="fs_cli -P 8022" ./test-live.sh
set -u

FS_CONF=${FS_CONF:-$HOME/fs}
FS_CLI=${FS_CLI:-fs_cli -P 8022}
# Path to FS_CONF as the FreeSWITCH process sees it; $${conf_dir} in the
# dialplan resolves to the same place.
CONF_DIR=${CONF_DIR:-$($FS_CLI -x 'global_getvar conf_dir')}
REPO=$(cd "$(dirname "$0")/../.." && pwd)
TARGET=${TARGET:-x86_64-unknown-linux-musl}
PROFILE=${PROFILE:-release-min}

fail=0
# Extension in sip_uri_test.xml that probe() routes to.
EXTEN=parse

api() { $FS_CLI -x "$*"; }

# Route one URI through the test dialplan and echo "var=value" per requested
# variable, using _undef_ for unset.
probe() {
	local uri=$1 leg dump
	shift
	drain
	$FS_CLI -x "originate {test_uri=$uri}loopback/$EXTEN/sip_uri_test &sleep(20000)" >/dev/null 2>&1 &
	for _ in $(seq 40); do
		leg=$(api 'show channels as json' | jq -r '.rows[]? | select(.name|endswith("-b")) | .uuid' | head -1)
		[ -n "$leg" ] && break
		sleep 0.1
	done
	if [ -z "$leg" ]; then
		echo "ERR no dialplan leg for $uri" >&2
		drain
		return 1
	fi
	# One snapshot: reading var by var races the channel's teardown.
	dump=$(api "uuid_dump $leg json")
	for var in "$@"; do
		printf '%s=%s\n' "$var" "$(jq -r --arg v "variable_$var" '.[$v] // "_undef_"' <<<"$dump")"
	done
}

# Hang up everything and wait for the channels to actually go away; hupall
# returns before teardown finishes and the next probe would find the corpse.
drain() {
	api 'hupall NORMAL_CLEARING' >/dev/null
	for _ in $(seq 40); do
		# The count is not on the first line of the reply.
		[ "$(api 'show channels count' | awk '/total\./ {print $1}')" = 0 ] && return
		sleep 0.1
	done
	echo "ERR channels still up after hupall" >&2
}

# check <uri> <var=expected>...
check() {
	local uri=$1 got want var
	shift
	local vars=()
	for want in "$@"; do vars+=("${want%%=*}"); done
	got=$(probe "$uri" "${vars[@]}") || { fail=1; return; }
	for want in "$@"; do
		var=${want%%=*}
		if grep -qxF "$want" <<<"$got"; then
			printf 'ok    %-52s %s\n' "$uri" "$want"
		else
			printf 'FAIL  %-52s want %s, got %s\n' "$uri" "$want" \
				"$(grep -E "^$var=" <<<"$got")"
			fail=1
		fi
	done
}

echo "== build"
cargo build --profile "$PROFILE" --manifest-path "$REPO/Cargo.toml" --target "$TARGET" --example fs-sip-uri || exit 1
install -Dm0755 "$REPO/target/$TARGET/$PROFILE/examples/fs-sip-uri" "$FS_CONF/bin/fs-sip-uri" || exit 1
install -Dm0644 "$(dirname "$0")/sip_uri_test.xml" "$FS_CONF/dialplan/sip_uri_test.xml" || exit 1
api reloadxml >/dev/null
# The default sessions-per-second ceiling throttles a run of back-to-back
# originates into failures that look like parse errors.
api 'fsctl sps 1000' >/dev/null

echo "== spawn_stream, no channel"
for probe_case in \
	"get user sip:+15551234567@sip.example.com;participantid=2|+15551234567" \
	"get user sip:sip.example.com;participantid=2|" \
	"get param.participantid sip:sip.example.com;participantid=2|2" \
	"get host sip:1411@[2001:db8::1]:5061|[2001:db8::1]" \
	"get nss urn:service:sos|sos"; do
	args=${probe_case%|*}
	want=${probe_case##*|}
	got=$(api "spawn_stream $CONF_DIR/bin/fs-sip-uri $args")
	got=${got%$'\n'}
	# fs_cli renders an empty API response as -ERR no reply.
	[ "$got" = "-ERR no reply" ] && got=""
	if [ "$got" = "$want" ]; then
		printf 'ok    %-52s -> [%s]\n' "$args" "$got"
	else
		printf 'FAIL  %-52s want [%s], got [%s]\n' "$args" "$want" "$got"
		fail=1
	fi
done

echo "== dialplan"
check 'sip:+15551234567@sip.example.com;participantid=2' \
	uri_type=sip uri_user=+15551234567 uri_host=sip.example.com \
	uri_param_participantid=2 inline_user=+15551234567 inline_participantid=2

check 'sip:sip.example.com;participantid=2' \
	uri_user=_undef_ uri_host=sip.example.com uri_param_participantid=2 \
	inline_user=_undef_ inline_participantid=2

check 'sip:sip.example.com;participantid=9f8e7d6c-1234' \
	uri_param_participantid=9f8e7d6c-1234

check 'sip:+15551234567;cpc=emergency@198.51.100.1;user=phone' \
	uri_user=+15551234567 uri_host=198.51.100.1 \
	uri_uparam_cpc=emergency uri_param_user=phone

check 'tel:+15551234567;cpc=emergency' \
	uri_type=tel uri_user=+15551234567 uri_param_cpc=emergency uri_host=_undef_

check 'urn:service:sos' uri_type=urn uri_nid=service uri_nss=sos

check '<sip:1305@pbx.example.com;user=phone>' uri_user=1305 uri_param_user=phone

check 'sip:1411@[2001:db8::1]:5061;user=phone' uri_host='[2001:db8::1]' uri_port=5061

# %3B decodes to ';' in the user part, so the payload must not use ';'.
check 'sip:a%3Bb@host.example.com' uri_user='a;b'

# multiset splits each pair on the first '=', so a value may contain one.
check 'sip:a%3Db@host.example.com' uri_user='a=b'

# Unparsable input sets nothing and logs at WARNING.
check 'garbage-not-a-uri' uri_type=_undef_ uri_host=_undef_ inline_user=_undef_

# A raw delimiter in a component aborts the payload rather than corrupting it.
check 'sip:a|b@host.example.com' uri_type=_undef_ inline_user='a|b'

# The first parse sets user, password, port and a param the second URI lacks;
# multiunset must leave none of them readable.
echo "== reparse clears the previous parse"
EXTEN=reparse
check 'sip:second.example.com;participantid=2' \
	uri_host=second.example.com uri_param_participantid=2 \
	uri_user=_undef_ uri_password=_undef_ uri_port=_undef_ uri_param_first=_undef_
EXTEN=parse

echo
[ $fail -eq 0 ] && echo "all passed" || echo "FAILURES"
exit $fail
