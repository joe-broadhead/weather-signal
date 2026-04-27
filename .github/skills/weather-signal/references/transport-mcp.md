# MCP Transport

Prefer MCP when Weather Signal tools are available in the client. MCP responses
return JSON text content with the same weather fields used by the CLI.

Use the CLI instead of MCP for cache mutation and shell-only operations such as
`cache prune`, `cache clear`, `places add`, and `places remove`.

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
The transport is stateless by default. Use `--http-stateful-mode` only for
trusted local clients because stateful sessions are held in process memory.

Health probes for streamable HTTP:

```bash
curl http://127.0.0.1:8768/healthz
curl http://127.0.0.1:8768/readyz
```

`--http-path` cannot be `/healthz` or `/readyz`.

## Error handling

MCP tool failures are surfaced as tool errors (`isError: true`). Do not treat a
failed tool call as usable weather evidence. Read the error text, correct the
location, date range, or threshold arguments, and retry only when appropriate.

## Reporting Standard

When summarizing MCP results, include:

- tool used
- resolved location and country code
- forecast dates or historical date range
- source
- cache state
- highest-impact weather fields or flags
