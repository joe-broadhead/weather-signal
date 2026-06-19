# Changelog

All notable changes to Weather Signal will be documented in this file.

This project follows semantic versioning once releases begin.

## Unreleased

## [0.0.1] - 2026-06-19

- Use native OS certificate roots with Rustls-backed HTTP requests so macOS
  Keychain and other platform trust stores are honored.
- Harden GitHub Actions workflows by avoiding persisted checkout credentials in
  read-only jobs and narrowing Pages deployment token permissions.
- Document repository security automation for vulnerability alerts, Dependabot
  security updates, secret scanning, and Rust supply-chain checks.
- Tighten the cargo-deny license allowlist by removing an unused license entry.

## [0.0.0] - 2026-04-27

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
- Release installer script with OS/architecture detection, checksum
  verification, and optional agent skill installation.
- Resilient bounded-concurrency batch signal output with per-location errors.
- Security policy, Dependabot configuration, cargo-audit CI, and expanded
  release targets.
