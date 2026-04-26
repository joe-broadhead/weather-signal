---
name: weather-demand-signals
description: Use Weather Signal MCP or CLI output as demand forecasting features. Use when a task asks to enrich product demand, staffing, inventory, marketing, or operational forecasts with weather context from weather-signal.
license: MIT
allowed-tools: "Bash Read"
metadata:
  owner: "weather-signal"
  version: "0.1.0"
---

# Weather Demand Signals Skill

## Mission

Convert weather context into useful forecasting features without overstating
what weather alone can explain.

## Required workflow

1. Prefer the `demand_signal` MCP tool when available; otherwise use
   `weather-signal signal` for the requested location and horizon.
2. Preserve the raw JSON as evidence when building downstream artifacts.
3. Flatten `days[]` to one row per location/date.
4. Join weather rows to business demand data by location/date.
5. Compare model or forecast behavior with and without weather features.
6. Report weather as an explanatory input, not as a standalone demand forecast.

## Default CLI Command

```bash
weather-signal signal <location> --days 7 --output json
```

For ambiguous place names, add a country hint:

```bash
weather-signal signal london --country GB --days 7 --output json
```

## Feature priority

Prioritize these fields:

- `temp_max_c`
- `temp_min_c`
- `apparent_temp_max_c`
- `precip_probability_max_pct`
- `precipitation_mm`
- `wind_gust_max_kmh`
- `sunshine_hours`
- `uv_index_max`
- `flags.rain_likely`
- `flags.wet_day`
- `flags.heavy_rain`
- `flags.warm_day`
- `flags.hot_day`
- `flags.cold_day`
- `flags.windy_day`
- `flags.sunny_day`
- `flags.high_uv`

## Guardrails

- Do not imply causality from weather features alone.
- Do not collapse multiple locations into one city forecast unless the user
  explicitly accepts that approximation.
- Include `fetched_at`, `cache`, and resolved `location` metadata in evidence.
- Use `--refresh` for operational decisions; use cache for repeatable analysis.

## When more detail is needed

Read:

- `.github/skills/weather-signal/references/transport-mcp.md`
- `.github/skills/weather-signal/references/transport-cli.md`
- `.github/skills/weather-signal/references/forecasting-workflows.md`
- `.github/skills/weather-signal/references/output-contracts.md`
