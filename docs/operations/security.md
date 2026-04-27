# Security

Weather Signal is designed for local agent and automation workflows. The CLI
does not require secrets when using the public Open-Meteo endpoints.

## API Keys

Commercial Open-Meteo deployments can pass an API key with `--api-key` or
`OPEN_METEO_API_KEY`.

```bash
export OPEN_METEO_API_KEY="..."
weather-signal signal london --country GB
```

Do not commit API keys to config files, docs, examples, or test fixtures. Help
output hides environment variable values for the API key, and diagnostics redact
the `apikey` query parameter.

## MCP HTTP

The streamable HTTP MCP server should stay on loopback unless an authenticating
proxy controls access.

```bash
weather-signal server start \
  --transport streamable-http \
  --http-host 127.0.0.1 \
  --http-port 8768 \
  --http-path /mcp
```

Binding to a public interface makes the weather tools available to any client
that can reach the server. Weather Signal does not implement application-level
authentication.

Streamable HTTP is stateless by default. Use `--http-stateful-mode` only for
trusted local clients because stateful sessions are held in process memory for
the lifetime of the server process.

## Local Files

Saved places are stored in:

```text
~/.config/weather-signal/config.toml
```

Cached API responses are stored in:

```text
~/.cache/weather-signal
```

Treat saved places and coordinates as business data. Use isolated
`XDG_CONFIG_HOME` and `XDG_CACHE_HOME` values for CI, shared runners, and
multi-tenant agent environments.

## Reporting Issues

See the top-level `SECURITY.md` for supported versions and vulnerability
reporting guidance.
