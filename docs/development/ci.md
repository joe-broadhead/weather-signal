# CI & Quality

CI runs on pull requests and pushes to `master` or `main`.

## Rust Checks

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all-features`
- `cargo deny check`
- `cargo audit`

The committed `rust-toolchain.toml` pins Rust 1.93.0 and is the source of truth
for the compiler used by local development and CI.

## Docs Checks

Docs use MkDocs Material and are built strictly:

```bash
mkdocs build --strict
```

The docs workflow publishes GitHub Pages from `master` or `main`.

## Release Automation

Release automation follows the same branch-and-tag model used in related
projects:

1. Run `Prepare Release` with a semantic version such as `0.0.0`.
2. Merge the generated `release/<version>` PR into `master` or `main`.
3. `Tag Release` creates and pushes `v<version>`.
4. `Release` validates metadata, reruns fmt/clippy/test/deny/audit gates,
   builds release binaries, generates checksums and SBOMs, and publishes GitHub
   release assets.

The release tag workflow requires `RELEASE_TAG_TOKEN` with `contents:write`.

See [Release](release.md) for the full release checklist.
