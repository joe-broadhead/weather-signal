# MCP Clients

Weather Signal exposes the same weather workflows through MCP tools. Use MCP
when an agent should call weather tools directly instead of shelling out.

## Stdio Client

Use stdio for local desktop agents and single-user automation.

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

If the binary is not on `PATH`, use the absolute path:

```json
{
  "mcpServers": {
    "weather-signal": {
      "command": "/usr/local/bin/weather-signal",
      "args": ["server", "start", "--transport", "stdio"]
    }
  }
}
```

## Streamable HTTP

Use streamable HTTP when a local service or agent runtime needs HTTP MCP.

```bash
weather-signal server start \
  --transport streamable-http \
  --http-host 127.0.0.1 \
  --http-port 8768 \
  --http-path /mcp
```

Health probes are available outside the MCP path:

```bash
curl http://127.0.0.1:8768/healthz
curl http://127.0.0.1:8768/readyz
```

Keep the server bound to loopback unless an authenticating proxy controls
access. The server warns when it is bound to a non-loopback host.

Streamable HTTP is stateless by default. Use `--http-stateful-mode` only for
trusted local clients because stateful sessions are held in process memory.

## Tool Selection

| Need | Start With | Then Use |
| --- | --- | --- |
| Broad planning context | `weather_summary` | `demand_signal` for daily features |
| Rain or heat trigger days | `threshold_days` | `daily_forecast` for raw values |
| Ambiguous place name | `geocode` | Pass the resolved place or coordinates |
| Backtesting context | `historical_weather` | Join with demand history |
| Cache inspection | `cache_status` | CLI `cache prune` for maintenance |

## Agent Guidance

- Prefer `weather_summary` before fetching detailed rows.
- Use `demand_signal` when the agent needs stable feature names.
- Use `threshold_days` when the workflow has a clear decision rule.
- Include a country hint for ambiguous cities.
- Treat MCP errors as tool failures and inspect the returned message before
  retrying.
