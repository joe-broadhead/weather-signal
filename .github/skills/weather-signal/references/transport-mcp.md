# MCP Transport

Prefer MCP when Weather Signal tools are available in the client. MCP responses
return JSON text content with the same weather fields used by the CLI.

## Tool Selection

| Need | Tool |
| --- | --- |
| Resolve an ambiguous place | `geocode` |
| Current conditions | `current_weather` |
| Daily variables | `daily_forecast` |
| Hourly variables | `hourly_forecast` |
| Demand features | `demand_signal` |
| Compact overview | `weather_summary` |
| Decision-rule matches | `threshold_days` |
| Backtesting features | `historical_weather` |
| Saved aliases | `list_places` |
| Cache inspection | `cache_status` |

## Common Calls

Use `weather_summary` first when the user wants a quick business-readable
overview:

```json
{
  "location": "london",
  "country": "GB",
  "days": 7
}
```

Use `demand_signal` when the output will be joined to demand, staffing,
inventory, marketing, or operational forecasts:

```json
{
  "location": "london",
  "country": "GB",
  "days": 7
}
```

Use `threshold_days` when the task asks which dates meet a rule:

```json
{
  "location": "london",
  "country": "GB",
  "days": 7,
  "rain_prob_gte": 60,
  "wind_gust_gte": 40
}
```

Use `historical_weather` for backtesting:

```json
{
  "location": "london",
  "country": "GB",
  "start": "2026-04-20",
  "end": "2026-04-25"
}
```

## Server Startup

For local MCP clients:

```bash
weather-signal server start --transport stdio
```

For hosted or shared agent runtimes:

```bash
weather-signal server start --transport streamable-http --http-host 127.0.0.1 --http-port 8768 --http-path /mcp
```

Keep streamable HTTP on loopback unless an authenticating proxy controls access.

## Reporting Standard

When summarizing MCP results, include:

- tool used
- resolved location and country code
- forecast dates or historical date range
- source
- cache state
- highest-impact weather fields or flags
