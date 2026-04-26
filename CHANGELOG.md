# Changelog

All notable changes to Weather Signal will be documented in this file.

This project follows semantic versioning once releases begin.

## Unreleased

## [0.0.0]

- Initial Rust CLI for Open-Meteo geocoding, current, daily, hourly, and demand
  signal outputs.
- JSON default output with table and CSV alternatives.
- Saved places, local response cache, and configurable Open-Meteo endpoints.
- Release automation with prepare, tag, and binary publish workflows.
- Agent skill for teaching forecasting agents how to use Weather Signal.
- `AGENTS.md` development guide for coding agents working in the repo.
- CLI hardening for HTTP timeouts, coordinate validation, geocode bounds, and
  explicit signal profile parsing.
- Agent workflow commands: `summary`, `threshold`, `batch signal`,
  `historical`, and `completions`.
- MCP server support over stdio and streamable HTTP with weather tools and
  health probes.
- Resilient bounded-concurrency batch signal output with per-location errors.
- Security policy, Dependabot configuration, cargo-audit CI, and expanded
  release targets.
