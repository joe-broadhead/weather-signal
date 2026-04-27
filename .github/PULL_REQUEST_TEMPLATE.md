## Summary

Describe the change and why it is needed.

## Checks

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --locked --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --all-features`
- [ ] `cargo deny check`
- [ ] `cargo audit`
- [ ] `mkdocs build --strict` if docs changed

## Release Notes

- [ ] `CHANGELOG.md` updated for user-visible changes
- [ ] README/docs updated for changed behavior
- [ ] No API keys, private endpoint URLs, or customer locations included
