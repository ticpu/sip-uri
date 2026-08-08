#!/bin/bash
# Checkout the release tag, publish to crates.io, return to the branch.
#
# Usage: scripts/release-publish.sh vX.Y.Z

set -e

VERSION="$1"
if [ -z "$VERSION" ]; then
	echo "Usage: $0 vX.Y.Z" >&2
	exit 1
fi

if ! git rev-parse -q --verify "refs/tags/$VERSION" >/dev/null; then
	echo "Tag $VERSION not found locally. Push/fetch it first." >&2
	exit 1
fi

BRANCH="$(git symbolic-ref --short HEAD)"

git checkout "$VERSION"
cargo publish --dry-run
cargo publish
git switch "$BRANCH"

echo "Published sip-uri $VERSION."
