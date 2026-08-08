Perform a release of sip-uri.

Optional override: $ARGUMENTS (format: vX.Y.Z). If provided, use that version.

## Version determination

1. Find the last release tag (`git tag --sort=-v:refname | head -1`).
2. Examine commits since that tag to classify the release type (0.x semver:
   the minor is the breaking axis):
   - **Patch** (0.Y.z+1): bug fixes, additive public API, dependency bumps,
     build changes, docs.
   - **Breaking** (0.Y+1.0): changed/removed public items, incompatible
     behavior changes. Stop and confirm before proceeding. 0.3.0 is reserved
     for the `NameAddr` removal.

## Pre-release checks

Run in sequence — stop and report on any failure:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --no-deps
cargo test --release
cargo semver-checks check-release
cargo publish --dry-run
```

## Steps

1. Bump `version` in `Cargo.toml`.

2. Run pre-release checks above.

3. Draft a changelog from `git log --oneline <last-tag>..HEAD`.

   **Rules:**
   - Group under: `New features:`, `Bug fixes:`, `Build:`, `Refactoring:` — omit empty sections.
   - Describe user-visible behavior, not implementation details.
   - Merge related commits for the same feature into one bullet.
   - No git hashes, no raw commit subjects, no co-author lines.

   Tag annotation format:
   ```
   vX.Y.Z

   New features:
   - what changed

   Bug fixes:
   - what was fixed

   Build:
   - what changed
   ```

4. Commit the bump and build the tag locally — nothing is pushed yet. The tag
   sits on a detached child commit that pins `Cargo.lock`, so the lock never
   lands on master while the released build still resolves an exact
   dependency set:

```sh
git add Cargo.toml
git commit -m "release: vX.Y.Z"
git checkout --detach
git add -f Cargo.lock
git commit -m "build: pin Cargo.lock for vX.Y.Z"
git tag -as vX.Y.Z -m "$(cat <<'EOF'
vX.Y.Z

<changelog>
EOF
)"
git switch master
```

   Run these as **separate** commands, never chained with `&&`. If a chained
   command is rejected part-way — a hook, a denied permission — the untried
   half is silently skipped, and the failure mode here is committing
   `Cargo.lock` onto master because the `git checkout --detach` never ran.
   After `git checkout --detach`, confirm with `git symbolic-ref -q HEAD`
   (it must fail) before staging the lock. The `pre-commit` hook rejects a
   staged `Cargo.lock` on a branch and `pre-push` rejects a branch tip that
   tracks it, but neither replaces checking that the detach took.

5. Push master, wait for CI green:

```sh
git push
gh run watch "$(gh run list --workflow=ci.yml -b master -L1 --json databaseId --jq '.[0].databaseId')" --exit-status
```

   No run within a couple of minutes: check the `Actions` component at
   `https://www.githubstatus.com/api/v2/components.json` — during an outage no
   run is created and missed events are never backfilled. Stop and report.

   Red: fix on master, rebuild the tag onto the new head, restart this step.

6. Push the tag:

```sh
git push origin vX.Y.Z
```

   The tag is IMMUTABLE once pushed — never retag. Wrong? Make a new patch
   release.

7. Publish, from the tagged commit:

```sh
git checkout vX.Y.Z
cargo publish
git switch master
```

   `git switch master` deletes the working-tree `Cargo.lock` (untracked there);
   the next cargo command regenerates it.

8. Report the tag, the changelog, the CI run that gated the publish, and the
   crates.io version (`curl https://index.crates.io/si/p-/sip-uri`).

## Important

- **Never publish a commit CI has not run on.** The tag's pin commit differs
  from the CI-green master tip only by `Cargo.lock`. If anything else changed
  after the checks — a rebase, a hand-resolved conflict — the earlier green
  run does not cover it. Re-run the checks and go back to step 5.
- **Ask before publishing when anything deviated from these steps.** An outage,
  a rebase, a skipped step, a red-then-fixed run: report the state and let me
  decide.
- **Cargo.lock never reaches master** — library crate, stays gitignored there.
  It exists only on the tag's own commit, so a release build is reproducible.
