# Release

Weather Signal uses a PR-gated release flow.

## Prepare

Update release metadata before preparing a tag:

1. Move user-visible changes from `CHANGELOG.md` `Unreleased` into the target
   version section.
2. Ensure `Cargo.toml` `version` matches the target version.
3. Run the local quality gates.

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo deny check
cargo audit
mkdocs build --strict
```

## Open the Release PR

Run the `Prepare Release` workflow with the target semantic version. The
workflow validates metadata, creates `release/<version>`, and opens a release
PR.

The release branch includes an empty marker commit so the PR is mergeable even
when all version metadata is already present on `master`.

## Tag and Publish

For a public launch, make the repository public before merging the release PR
that creates the tag. The release workflow only publishes build provenance
attestations when GitHub reports the repository as public, and the documented
installer URLs use public raw GitHub and release asset URLs.

After the release PR is merged, the `Tag Release` workflow creates `v<version>`.
The `Release` workflow then:

- validates the tag against `Cargo.toml`,
- checks that `CHANGELOG.md` has a matching version section,
- reruns fmt, clippy, tests, deny, and audit,
- builds release binaries,
- publishes checksums, SBOMs, and provenance attestations,
- creates the GitHub release.

The tag workflow requires `RELEASE_TAG_TOKEN` with `contents:write`.
