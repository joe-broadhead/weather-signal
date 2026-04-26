# AGENTS.md

Guidance for coding agents working in this repository.

## Project Overview

`weather-signal` is a Rust CLI and MCP server for turning Open-Meteo weather
data into agent-ready forecasts and demand signal features. It is designed for
scripts, forecasting agents, and business workflows that need stable JSON,
saved locations, cache-aware execution, and simple derived weather flags.

Core areas:

- `src/main.rs` - binary entrypoint and command dispatch.
- `src/cli.rs` - Clap command, option, and parser definitions.
- `src/app.rs` - Open-Meteo client workflows and location resolution.
- `src/cache.rs` - local HTTP response cache.
- `src/models.rs` - shared data contracts and response envelopes.
- `src/signals.rs` - demand signals, threshold matching, and summaries.
- `src/output.rs` - JSON/table/CSV rendering.
- `src/mcp.rs` - MCP stdio and streamable HTTP transports.
- `src/util.rs` - small parsing and path helpers.
- `.github/skills/` - packaged agent skills for CLI usage, demand-signal
  workflows, and saved-location setup.
- `.github/workflows/` - CI, docs, release prepare/tag/publish automation.
- `docs/` - MkDocs Material documentation site.
- `README.md` - public project overview and quick start.
- `CONTRIBUTING.md` - contributor workflow and release expectations.

## High-Signal Commands

Run focused checks while developing, then run the relevant release-grade checks
before handoff.

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
mkdocs build --strict
cargo deny check
```

Useful runtime smoke checks:

```bash
cargo run -- signal london --country GB --days 3
cargo run -- summary london --country GB --days 3
cargo run -- threshold london --country GB --days 3 --rain-prob-gte 50
cargo run -- geocode london --country GB --count 1 --table
cargo run -- daily london --country GB --days 2 --output csv
```

The runtime smoke checks call Open-Meteo and require network access. Do not put
live-network tests into CI unless the user explicitly asks for that tradeoff.

When editing workflow files, use `actionlint` if it is available:

```bash
actionlint .github/workflows/<workflow>.yml
```

## Development Rules

- Preserve the public CLI contract unless the user asks for a breaking change.
- JSON is the default public interface. Avoid renaming JSON fields casually.
- Keep table and CSV output useful, but treat JSON as the source-of-truth shape.
- Echo resolved location metadata in output when adding forecast-like commands.
- Keep Open-Meteo request construction centralized and auditable.
- Use `--country` or `geocode` guidance for ambiguous place names.
- Add or update tests for behavior changes.
- Prefer explicit error propagation over panics or silent fallback.
- Do not add `.expect()` or `.unwrap()` in production paths unless the invariant
  is strong and the message is useful.
- Keep source and docs ASCII unless external data or branding requires
  non-ASCII.
- Do not commit generated build artifacts, caches, local config, or `site/`.

## Rust Notes

- Minimum supported Rust version is 1.93.
- Edition is 2024.
- `Cargo.lock` is committed because this is a binary crate.
- TLS should stay on `rustls` unless there is a strong reason to change it.
- Prefer small, boring dependencies. New runtime dependencies should justify
  their maintenance and supply-chain cost.
- Keep module boundaries behavioral: CLI parsing in `cli`, HTTP/weather
  workflows in `app`, cache mechanics in `cache`, output formatting in
  `output`, signal derivation in `signals`, MCP transport/tool routing in
  `mcp`, and shared contracts in `models`.

## CLI Contract Rules

Supported command families:

- `geocode` for location discovery
- `places` for saved business-location aliases
- `current`, `daily`, and `hourly` for direct weather data
- `signal` for demand-oriented daily features
- `summary` for compact agent overviews of a signal window
- `threshold` for filtering days that match decision rules
- `batch signal` for saved places or CSV location lists
- `historical` for archive data used in backtests and feature engineering
- `server start` for MCP stdio or streamable HTTP transports
- `completions` for shell integration
- `cache` for local cache inspection and clearing

Location resolution order must remain:

1. saved alias
2. `lat,lon`
3. Open-Meteo geocoding

When changing `signal`, keep docs and skills aligned with the default demand
flag rules:

- `rain_likely`: precipitation probability max >= 50%
- `wet_day`: precipitation sum >= 1 mm
- `heavy_rain`: precipitation sum >= 5 mm
- `warm_day`: max temperature >= 20 C
- `hot_day`: max temperature >= 25 C
- `cold_day`: min temperature <= 5 C
- `windy_day`: wind gust max >= 40 km/h
- `sunny_day`: sunshine duration >= 6 hours
- `high_uv`: UV index >= 6

## Configuration and Cache Rules

Default paths:

- Config: `~/.config/weather-signal/config.toml`
- Cache: `~/.cache/weather-signal`

Rules:

- Keep config human-readable TOML.
- Do not store API keys in config examples or tests.
- Cache keys must include enough request context to prevent cross-endpoint or
  schema collisions.
- `--refresh` should bypass cache reads and write the fresh response.
- Network requests should retain a finite timeout so CLI calls cannot hang
  indefinitely in agent loops.
- Commercial endpoint behavior should use existing base URL/API key surfaces:
  `OPEN_METEO_API_KEY`, `OPEN_METEO_FORECAST_BASE_URL`,
  `OPEN_METEO_GEOCODING_BASE_URL`, `OPEN_METEO_HISTORICAL_BASE_URL`, or
  matching CLI flags.

## Documentation Rules

MkDocs is strict. If you change CLI behavior, update the matching docs:

- `README.md`
- `docs/reference/cli.md`
- `docs/reference/signals.md`
- `docs/reference/configuration.md`
- `docs/getting-started/quickstart.md`

Keep docs concise and example-driven. Avoid documenting private endpoints,
customer locations, or API keys.

## Agent Skill Rules

Skills live under `.github/skills/`.

Current skills:

- `weather-signal` - core MCP/CLI usage and command selection
- `weather-demand-signals` - demand forecasting feature workflows
- `weather-location-setup` - saved place and geocoding setup

When changing CLI commands, output fields, cache semantics, or signal flags,
update the relevant skill and reference file in the same change.

When changing MCP tools or transports, update `docs/reference/mcp.md`, the
README MCP section, and the protocol smoke tests.

Skill files should stay concise. Put reusable detail in one-level-deep
`references/` files rather than expanding `SKILL.md` indefinitely.

## Release Rules

- Do not use `release/*` or `hotfix/*` branch names unless the user intends to
  trigger the auto-tag release flow after merge.
- Release tags are `vX.Y.Z`.
- `release.yml` validates `Cargo.toml` and `CHANGELOG.md` against the tag.
- `release-prepare.yml` opens `release/X.Y.Z` PRs after metadata validation.
- `release-tag.yml` requires the repo secret `RELEASE_TAG_TOKEN` with
  `contents:write`.
- Do not merge release PRs without explicit user approval.
- For user-visible behavior, update `CHANGELOG.md`.

## Security and Privacy

- Never log or commit API keys, tokens, customer locations, private endpoint
  URLs, or raw business demand data.
- Keep public docs and tests synthetic or generic.
- Avoid live-network CI tests unless explicitly requested.
- Treat weather outputs as context features, not as operational guarantees.

## PR Checklist for Agents

Before opening or updating a PR:

- Run formatting and lint checks when code changed.
- Run tests for changed behavior.
- Run `mkdocs build --strict` when docs, README, or skills changed.
- Update `CHANGELOG.md` for user-facing behavior, release automation, docs, or
  skill changes.
- Include validation commands in the PR body.
- Call out checks that were intentionally skipped and why.
