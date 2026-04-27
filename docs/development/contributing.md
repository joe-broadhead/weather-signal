# Contributing

The repository-level contribution guide has the full workflow:
[CONTRIBUTING.md](https://github.com/joe-broadhead/weather-signal/blob/master/CONTRIBUTING.md).

Local quality gates:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
mkdocs build --strict
```

Release flow is documented in [Release](release.md).
