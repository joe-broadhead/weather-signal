# Usage Patterns

Weather Signal has three independent choices that shape a workflow:

1. How locations are identified.
2. Which weather surface is used.
3. Which output transport is consumed.

Use the narrowest combination that answers the forecasting question. Start with
`summary` for context, move to `signal` or `threshold` for row-level features,
and use `daily`, `hourly`, or `historical` when you need raw weather variables.

## Location Inputs

| Mode | Example | Best For |
| --- | --- | --- |
| Saved alias | `weather-signal signal london` | Recurring business locations |
| Place search | `weather-signal signal "London" --country GB` | One-off city or region lookups |
| Coordinates | `weather-signal daily "51.5074,-0.1278"` | Deterministic joins and known sites |
| CSV batch | `weather-signal batch signal --input locations.csv` | Multi-site forecasting runs |

Saved aliases are the most stable option for agent workflows because the
resolved location is explicit in every response and can be reviewed in config.

## Weather Surfaces

| Surface | Command | Use When |
| --- | --- | --- |
| Current weather | `current` | A workflow needs the current conditions only |
| Forecast variables | `daily`, `hourly` | You need raw Open-Meteo fields |
| Demand features | `signal` | You need daily features that join cleanly to demand data |
| Forecast summary | `summary` | You need a compact overview for planning or agent context |
| Decision rules | `threshold` | You need days that match specific weather triggers |
| Historical archive | `historical` | You need backtesting or feature-store inputs |

## Output Transports

| Transport | Example | Best For |
| --- | --- | --- |
| CLI JSON | `weather-signal signal london` | Agents, scripts, and repeatable jobs |
| CLI table | `weather-signal daily london --table` | Human inspection |
| CLI CSV | `weather-signal hourly london --output csv` | Spreadsheets and feature stores |
| MCP stdio | `weather-signal server start --transport stdio` | Local MCP clients |
| MCP streamable HTTP | `weather-signal server start --transport streamable-http` | Local services behind an auth proxy |

## Recommended Profiles

| Profile | Location | Command | Output |
| --- | --- | --- | --- |
| Single-site planning | Saved alias | `summary` | JSON |
| Feature engineering | Coordinates or saved alias | `signal` | JSON |
| Trigger-based operations | Saved alias | `threshold` | JSON |
| Multi-site demand forecast | CSV or saved places | `batch signal` | JSON |
| Analyst exploration | Place search | `daily --table` | Table |
| Backtesting | Coordinates | `historical --output csv` | CSV |

## Validation Checklist

```bash
weather-signal geocode london --country GB --count 1
weather-signal places add london "London" --country GB
weather-signal summary london --days 7
weather-signal threshold london --days 7 --rain-prob-gte 60
weather-signal cache status
```

For production agent loops, prefer saved aliases or coordinates, keep stdout in
JSON, and route logs from stderr separately.
