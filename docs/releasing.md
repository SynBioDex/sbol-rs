# Release process

This workspace releases all twelve crates at one shared version. The
`sbol-py` package is versioned with the workspace but is not published
to crates.io. Pushing a `v*` tag builds the cross-platform CLI archives
and creates the GitHub release; publishing crates.io packages remains
an explicit maintainer action.

## Prepare the release commit

1. Start from a clean `master` at the commit that passed CI.
2. Set the release version in every publishable crate manifest, every
   internal dependency requirement in the root `Cargo.toml`,
   `crates/sbol-py/Cargo.toml`, and
   `crates/sbol-py/pyproject.toml`.
3. Move the release notes out of `Unreleased`, add the release date,
   and update the changelog comparison links.
4. Update the README installation version and `CITATION.cff` version
   and release date. Keep the repository's Zenodo concept DOI in the
   release commit; the version-specific DOI does not exist until
   Zenodo has archived the GitHub release.
5. Refresh all three lockfiles without broad dependency upgrades:

   ```sh
   cargo check --workspace
   cargo check --manifest-path fuzz/Cargo.toml
   cargo check --manifest-path crates/sbol-py/Cargo.toml
   ```

## Validate

Run the same gates the release depends on:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo publish --dry-run --workspace --locked
```

Also build and test the excluded Python package according to
[`python.md`](python.md). Push the release commit and wait for every CI
job on that exact commit to pass before publishing or tagging it.

## Publish and tag

All crates.io packages are published by the same owner and Cargo 1.93
can publish the workspace in dependency order:

```sh
cargo publish --workspace --locked
```

Cargo waits for each uploaded package to appear in the registry index
before publishing dependents. If that wait times out, check crates.io
before retrying; a timeout does not mean the upload failed.

After all twelve `1.0.0` packages are visible on crates.io, create and
push an annotated tag from the already-tested release commit:

```sh
git tag -a v1.0.0 -m v1.0.0
git push origin v1.0.0
```

The release workflow verifies that the tag, package manifests,
changelog, README, Python metadata, and citation metadata agree. It
then builds four CLI archives and creates the GitHub release.

## Post-release checks

1. Confirm all twelve crates and their docs.rs builds are available.
2. Download and smoke-test at least one GitHub CLI archive.
3. Confirm Zenodo archived the GitHub release and that the concept DOI
   resolves to the new version.
4. If a version-specific DOI is desired in citation output, update
   `CITATION.cff` only after Zenodo has minted it.
5. Add the next development notes under `Unreleased`.
