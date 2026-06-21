---
name: weather-signal
description: Open-Meteo weather forecasts, geocoding, saved locations, demand signal features, and threshold filtering via MCP + CLI. Use when a task needs current/daily/hourly weather, weather-derived demand features for forecasting, city geocoding, or repeatable business-location aliases.
---

# Weather Signal

Weather and demand-signal features via Open-Meteo. Prefer MCP tools when available; fall back to the `weather-signal` CLI.

## Tool Map

All MCP tools are prefixed `weather-signal_`:

| Need | MCP Tool |
|------|----------|
| Current conditions | `current_weather` |
| Daily forecast (1-16d) | `daily_forecast` |
| Hourly forecast (1-384h) | `hourly_forecast` |
| Compact summary + risk/warm/wet flags | `weather_summary` |
| Demand forecasting features | `demand_signal` |
| Filter days by thresholds | `threshold_days` |
| Historical/archive data | `historical_weather` |
| Resolve place names | `geocode` |
| Saved location aliases | `list_places` |
| Cache inspection | `cache_status` |

## Escalation Pattern

1. `weather_summary` — compact overview (risk days, warm/hot/wet/windy counts)
2. `demand_signal` — feature enrichment for forecasting
3. `daily_forecast` / `hourly_forecast` — raw weather variables
4. `threshold_days` — filter to days matching decision rules
5. `current_weather` — now/current context
6. `historical_weather` — backtesting and feature engineering

## Defaults

- Output: JSON
- Forecast horizon: 7 days
- Hourly horizon: 48 hours
- Country hint for London: `--country GB`
- Cache: prefer cached for repeatable analysis; use `--refresh` for live decisions

## Guardrails

- Weather signals are **features for forecasting**, not standalone demand forecasts.
- Ambiguous place names: use `geocode` with `--country`, never guess London/Paris/Springfield.
- Saved locations: MCP can list but cannot add or remove — use CLI for mutations.
- For batch output, treat item-level `error` values as partial failures.
- MCP HTTP server loopback-only unless an authenticating proxy controls access.

## Sub-topics

- **Demand features**: forecast enrichment for demand, staffing, inventory models. See [references/demand-signals.md](references/demand-signals.md).
- **Location setup**: geocode, save places, validate aliases (CLI-only write). See [references/locations.md](references/locations.md).
- **CLI transport**: full `weather-signal` CLI command reference. See [references/cli.md](references/cli.md).
