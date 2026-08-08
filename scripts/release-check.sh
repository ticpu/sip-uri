#!/bin/bash
# Pre-release checks: fmt, clippy, docs, tests, semver, publish dry-run.
# Run on a clean master before scripts/release-tag.sh.

set -e

cargo fmt --all
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --no-deps
cargo test --release
cargo semver-checks check-release
cargo publish --dry-run

echo "Pre-release checks passed"
