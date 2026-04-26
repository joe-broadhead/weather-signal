# Contributing to Weather Signal

Thanks for contributing to Weather Signal. This guide describes the local
workflow, code standards, and expectations for high-quality PRs.

## Scope

We welcome:

- Bug fixes and reliability improvements
- New Open-Meteo variables or signal features
- Better CLI output formats and examples
- Documentation improvements
- Tests that catch regressions or clarify behavior

If the change is large, open an issue first so we can align on direction.

## Development Setup

```bash
git clone https://github.com/joe-broadhead/weather-signal.git
cd weather-signal

cargo build
cargo test
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --check
```

Docs validation:

```bash
python -m pip install -r docs/requirements.txt
mkdocs build --strict
```

## Branching Model

This repo uses a `master`-first release flow:

- Default branch: `master`
- Feature branches: `feature/<name>` off `master`
- Hotfix branches: `hotfix/<version>` off `master`
- Release tags: `v<version>` from `master`

Release flow:

1. Move user-visible changes from `CHANGELOG.md` `Unreleased` into
   `## [x.y.z]`.
2. Ensure `Cargo.toml` version matches `x.y.z`.
3. Run the `Prepare Release` workflow with `version=x.y.z`.
4. Merge the generated `release/x.y.z` PR.
5. The `Tag Release` workflow creates `vx.y.z`.
6. The `Release` workflow builds binaries and publishes GitHub release assets.

Release tagging requires the repository secret `RELEASE_TAG_TOKEN`, set to a PAT
or GitHub App token with `contents:write`.

## Code Standards

- Keep the CLI contract stable and script-friendly.
- JSON output is the default public interface; avoid breaking field names.
- Prefer explicit errors over panics or silent fallback.
- Avoid `.expect()` in production code unless the invariant is impossible to
  violate and documented locally.
- Keep source and docs ASCII unless non-ASCII is required by external data.
- Keep Open-Meteo requests centralized so variable lists, cache keys, and
  commercial endpoint handling remain auditable.

## Testing Guidance

All new behavior should include focused tests. Prefer fixture or mock-server
tests for API parsing rather than live network tests in CI.

Recommended local checks:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo deny check
cargo run -- signal london --country GB --days 3
```

The live `cargo run` check is optional and requires network access.

## Dependency Policy

- `Cargo.lock` is committed because this is a binary crate.
- Dependency changes should be intentional and included in the same PR as the
  `Cargo.lock` update.
- Prefer `rustls` TLS dependencies for portable CI and release builds.
- Avoid adding runtime dependencies for formatting or parsing that can be
  handled by the existing stack.

## Pull Request Checklist

- [ ] Tests pass (`cargo test --locked --all-features`)
- [ ] Lint passes (`cargo clippy --locked --all-targets --all-features -- -D warnings`)
- [ ] Formatting is clean (`cargo fmt --check`)
- [ ] Supply-chain checks pass (`cargo deny check`)
- [ ] Docs build if docs changed (`mkdocs build --strict`)
- [ ] README/docs updated if user-facing behavior changed
- [ ] `CHANGELOG.md` updated for notable changes
- [ ] No new panics in production code

## Security

Do not include API keys, private endpoint URLs, or customer location data in
issues, tests, fixtures, or docs examples. If you believe you found a security
issue, avoid public issues and use GitHub Security Advisories if available.
