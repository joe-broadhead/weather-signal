# Security Policy

## Reporting a Vulnerability

Please report security issues privately through GitHub Security Advisories for
the repository. If advisories are unavailable, contact the maintainer through
the repository owner's GitHub profile before opening a public issue. Do not put
vulnerability details in public issues.

Do not include API keys, private endpoint URLs, customer locations, or business
demand data in public issues, pull requests, logs, screenshots, or examples.

## Repository Security Automation

GitHub vulnerability alerts, Dependabot security updates, secret scanning, and
secret push protection should stay enabled for this repository. Dependabot
configuration lives in `.github/dependabot.yml`.

CI and release validation also run `cargo deny check` and `cargo audit` so
dependency policy and RustSec advisories are checked before changes ship.

## Supported Versions

Until the first stable release, security fixes target the latest published
release and the default branch.

## MCP HTTP

The streamable HTTP MCP transport has no built-in authentication. Keep it bound
to loopback or place it behind an authenticating reverse proxy before exposing
it to a network.

Streamable HTTP is stateless by default. Use `--http-stateful-mode` only for
trusted local clients because stateful sessions are held in process memory.
