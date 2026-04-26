# MCP Reference

Weather Signal exposes the same weather workflows through MCP for agents
that prefer tools over shell commands.

## Stdio

```bash
weather-signal server start --transport stdio
```

Example client config:

```json
{
  "mcpServers": {
    "weather-signal": {
      "command": "weather-signal",
      "args": ["server", "start", "--transport", "stdio"]
    }
  }
}
```

## Streamable HTTP

```bash
weather-signal server start \
  --transport streamable-http \
  --http-host 127.0.0.1 \
  --http-port 8768 \
  --http-path /mcp
```

Health probes:

```bash
curl http://127.0.0.1:8768/healthz
curl http://127.0.0.1:8768/readyz
```

Keep HTTP bound to loopback unless an authenticating proxy controls access. The
server prints a warning when `--http-host` is not `127.0.0.1`, `localhost`, or
`::1`.

## Tools

| Tool | Use |
| --- | --- |
| `geocode` | Resolve ambiguous place names |
| `current_weather` | Fetch current weather fields |
| `daily_forecast` | Fetch daily forecast variables |
| `hourly_forecast` | Fetch hourly forecast variables |
| `demand_signal` | Return daily demand-oriented weather features |
| `weather_summary` | Return compact counts and headline over the forecast window |
| `threshold_days` | Filter days by rain, precipitation, heat, cold, or wind rules |
| `historical_weather` | Fetch archive data for backtesting |
| `list_places` | List saved aliases |
| `cache_status` | Inspect local response cache state |

Prefer `weather_summary` first for broad business context, then call
`demand_signal` or `threshold_days` when the agent needs row-level features or
decision-rule matches.
