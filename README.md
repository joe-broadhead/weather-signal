# Weather Signal

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.93%2B-orange.svg?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Docs](https://img.shields.io/badge/docs-mkdocs%20material-blue.svg?logo=materialformkdocs&logoColor=white)](https://joe-broadhead.github.io/weather-signal/)
[![CI](https://github.com/joe-broadhead/weather-signal/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/joe-broadhead/weather-signal/actions/workflows/ci.yml)

<pre>
 _       __           __  __                 _____ _                   __
| |     / /__  ____ _/ /_/ /_  ___  _____   / ___/(_)___ _____  ____ _/ /
| | /| / / _ \/ __ `/ __/ __ \/ _ \/ ___/   \__ \/ / __ `/ __ \/ __ `/ /
| |/ |/ /  __/ /_/ / /_/ / / /  __/ /      ___/ / / /_/ / / / / /_/ / /
|__/|__/\___/\__,_/\__/_/ /_/\___/_/      /____/_/\__, /_/ /_/\__,_/_/
                                                 /____/
             Weather context
         for forecasting agents.
</pre>

Weather Signal is an **agent-first weather data CLI and MCP server** for turning
Open-Meteo forecasts into stable, scriptable demand signals. It is built for
forecasting flows that need more than a pretty terminal forecast: consistent
JSON, saved business locations, local caching, MCP tools, and demand-friendly
features such as rain likelihood, warm days, windy days, sunshine, and UV.

## What It Does

- **Fetches Open-Meteo forecasts** with no key required for local development.
- **Resolves city names** through Open-Meteo geocoding, with optional country hints.
- **Stores saved places** so business locations can be referenced by alias.
- **Outputs agent-ready JSON by default**, plus table and CSV modes.
- **Caches responses locally** to keep repeated agent runs fast and predictable.
- **Adds agent workflow commands** for summaries, thresholds, batch locations,
  historical context, and shell completions.
- **Supports commercial endpoints** through configurable base URLs and API keys.

## 30-Second Example

```bash
weather-signal signal london --country GB --days 3
```

Example output shape:

```json
{
  "source": "open-meteo",
  "location": {
    "name": "London",
    "country_code": "GB",
    "latitude": 51.50853,
    "longitude": -0.12574
  },
  "cache": "miss",
  "profile": "demand",
  "days": [
    {
      "date": "2026-04-27",
      "temp_max_c": 20.4,
      "precip_probability_max_pct": 46,
      "precipitation_mm": 0.1,
      "flags": {
        "rain_likely": false,
        "warm_day": true,
        "sunny_day": true
      }
    }
  ]
}
```

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/weather-signal/master/scripts/install.sh | bash
```

Install the binary and Weather Signal agent skills:

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/weather-signal/master/scripts/install.sh | bash -s -- --install-skills
```

### Prebuilt Binaries

Release assets are published from the GitHub Release workflow with checksums,
SBOMs, and provenance attestations.

```bash
# macOS Apple Silicon
curl -L -o weather-signal.tar.gz \
  https://github.com/joe-broadhead/weather-signal/releases/download/v0.0.0/weather-signal-aarch64-apple-darwin.tar.gz

tar -xzf weather-signal.tar.gz
./weather-signal --version
```

Choose the asset for your platform from
[Releases](https://github.com/joe-broadhead/weather-signal/releases).

### From Source

```bash
git clone https://github.com/joe-broadhead/weather-signal.git
cd weather-signal
cargo build --locked --release

# Try the main agent signal command
cargo run -- signal london --country GB --days 7

# Human-readable table
cargo run -- daily london --country GB --days 3 --table

# CSV for spreadsheets or feature stores
cargo run -- hourly "51.5072,-0.1276" --hours 24 --output csv
```

## Quick Start

```bash
weather-signal signal london --country GB --days 7
weather-signal daily london --country GB --days 3 --table
weather-signal hourly "51.5072,-0.1276" --hours 24 --output csv
```

## CLI Usage

```bash
weather-signal geocode <query> [--country GB] [--count 5]
weather-signal places add <alias> <query> [--country GB]
weather-signal places list
weather-signal places remove <alias>
weather-signal current <location>
weather-signal daily <location> [--days 7]
weather-signal hourly <location> [--hours 48]
weather-signal signal <location> [--days 7] [--profile demand]
weather-signal summary <location> [--days 7]
weather-signal threshold <location> --rain-prob-gte 60
weather-signal batch signal --places all
weather-signal historical <location> --start YYYY-MM-DD --end YYYY-MM-DD
weather-signal server start --transport stdio
weather-signal server start --transport streamable-http --http-port 8768
weather-signal completions zsh
weather-signal cache status
weather-signal cache prune --max-age 7d
weather-signal cache clear
```

Global options:

```bash
--output json|table|csv
--table
--refresh
--cache-ttl 30m
--timeout 30s
--api-key <key>
--forecast-base-url <url>
--geocode-base-url <url>
--historical-base-url <url>
--config <path>
```

## Saved Places

Saved places make repeatable business workflows less brittle:

```bash
weather-signal places add london "London" --country GB
weather-signal signal london --days 7
```

Config lives at:

```text
~/.config/weather-signal/config.toml
```

Cache lives at:

```text
~/.cache/weather-signal
```

Use `weather-signal cache prune --max-age 7d` for routine cache maintenance, or
`weather-signal cache clear` when you need to remove every cached response.

## Commercial Open-Meteo Usage

The public Open-Meteo endpoint is the default. For commercial or self-hosted
usage, configure an API key or endpoint override:

```bash
export OPEN_METEO_API_KEY="..."
export OPEN_METEO_FORECAST_BASE_URL="https://customer-api.open-meteo.com/v1/forecast"
export OPEN_METEO_GEOCODING_BASE_URL="https://geocoding-api.open-meteo.com/v1/search"
export OPEN_METEO_HISTORICAL_BASE_URL="https://archive-api.open-meteo.com/v1/archive"
```

## MCP Server

Weather Signal can run as an MCP server over stdio or streamable HTTP:

```bash
weather-signal server start --transport stdio
weather-signal server start --transport streamable-http --http-host 127.0.0.1 --http-port 8768 --http-path /mcp
```

The MCP surface exposes tools such as `weather_summary`, `demand_signal`,
`threshold_days`, `historical_weather`, `daily_forecast`, and `geocode`.
Streamable HTTP is stateless by default and should stay on loopback unless an
authenticating proxy controls access. Use `--http-stateful-mode` only for
trusted local clients because stateful sessions are held in process memory.

## Documentation

- [Getting Started](docs/getting-started/quickstart.md)
- [CLI Reference](docs/reference/cli.md)
- [MCP Reference](docs/reference/mcp.md)
- [Signal Reference](docs/reference/signals.md)
- [Configuration](docs/reference/configuration.md)
- [Agent Skills](docs/development/agent-skills.md)
- [Agent Development Guide](AGENTS.md)
- [Contributing](CONTRIBUTING.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, quality gates,
documentation checks, and PR expectations.

## License

MIT. See [LICENSE](LICENSE).
