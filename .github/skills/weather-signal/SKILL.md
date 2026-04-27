---
name: weather-signal
description: Use Weather Signal to fetch Open-Meteo weather forecasts, resolve locations, manage saved places, run MCP tools, and produce agent-ready weather and demand signal features for forecasting workflows. Use when a task needs current/daily/hourly weather context, weather-derived demand features, city geocoding, repeatable business-location aliases, MCP tools, or JSON/CSV/table outputs from the weather-signal binary.
license: MIT
allowed-tools: "Bash Read mcp__weather_signal__geocode mcp__weather_signal__current_weather mcp__weather_signal__daily_forecast mcp__weather_signal__hourly_forecast mcp__weather_signal__demand_signal mcp__weather_signal__weather_summary mcp__weather_signal__threshold_days mcp__weather_signal__historical_weather mcp__weather_signal__list_places mcp__weather_signal__cache_status"
metadata:
  owner: "weather-signal"
  version: "0.0.0"
---

# Weather Signal Skill

## Mission

Use Weather Signal to turn weather questions into reproducible, agent-ready
forecast signals through either MCP tools or the `weather-signal` CLI.

## Required workflow

1. Choose transport:
   - Prefer MCP when Weather Signal MCP tools are available in the client.
   - Otherwise use the `weather-signal` CLI through Bash.
2. Resolve location intent:
   - saved alias if the user gives a business/site name
   - `lat,lon` if provided
   - geocoding with `--country` when the city is ambiguous
3. Choose the smallest command that answers the task:
   - `summary` for compact forecast-window overview
   - `signal` for demand forecasting features
   - `threshold` for days matching decision rules
   - `batch signal` for multiple saved or CSV locations
   - `historical` for backtesting and feature engineering
   - `daily` for daily weather variables
   - `hourly` for sub-day detail
   - `current` for now/current context
   - `geocode` for location discovery
4. Prefer cached responses for repeatable analysis; use `--refresh` only when
   stale cache would materially affect the answer.
5. Report the resolved location, date range, source, cache state, and any caveat
   about ambiguous locations or low relevance to the business question.
6. Do not mix MCP and CLI in one answer unless debugging transport behavior or
   filling an intentional gap such as cache pruning, which is CLI-only.

## Command defaults

Use these defaults unless the user asks otherwise:

- Output: JSON
- Forecast horizon: 7 days for `signal` and `daily`
- Hourly horizon: 48 hours
- Country hint for London: `--country GB`
- Demand forecasting: prefer `signal --profile demand`
- Batch concurrency: start with `--concurrency 4`; lower it when rate limits
  matter.

## Transport Selection

- MCP: read `references/transport-mcp.md`.
- CLI: read `references/transport-cli.md`.

## Guardrails

- Do not treat weather signals as demand forecasts by themselves. They are
  features for a forecasting workflow.
- Do not silently assume the wrong London/Paris/Springfield. Use `geocode` or a
  country hint when a place name is ambiguous.
- Do not hardcode API keys. Use `OPEN_METEO_API_KEY` or `--api-key`.
- Do not echo API keys or credential-bearing base URLs in answers.
- Prefer cached responses for repeatable agent loops; use `--refresh` for
  operational decisions that need current API state.
- For commercial Open-Meteo deployments, use configured base URLs rather than
  changing command semantics.
- For MCP HTTP, keep the server on loopback unless an authenticating proxy
  controls access.
- Treat `batch signal` item-level `error` values as partial failures, not as a
  reason to discard successful items.
- Treat MCP tool failures as MCP errors (`isError: true`) and inspect the error
  text before retrying.

## Output contract

When summarizing results for the user, include:

- tool or command used
- resolved location and country code
- forecast dates or hourly window
- source (`open-meteo`)
- cache state (`hit`, `miss`, or `refresh`)
- highest-impact signal fields or flags

## References

Load only what is needed:

- `references/transport-mcp.md` for MCP tool usage.
- `references/transport-cli.md` for CLI command usage.
- `references/output-contracts.md` for JSON fields and signal meanings.
- `references/forecasting-workflows.md` for demand forecasting feature usage.
