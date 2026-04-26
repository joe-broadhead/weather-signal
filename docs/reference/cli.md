# CLI Reference

## Global Options

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

Global options may be passed before or after subcommands.

## Commands

### `geocode`

```bash
weather-signal geocode <query> [--country GB] [--count 5]
```

Searches Open-Meteo geocoding and returns candidate locations.

### `places`

```bash
weather-signal places add <alias> <query> [--country GB]
weather-signal places list
weather-signal places remove <alias>
```

Manages saved aliases in the local config file.

### `current`

```bash
weather-signal current <location>
```

Returns current weather fields for the resolved location.

### `daily`

```bash
weather-signal daily <location> [--days 7]
```

Returns daily forecast variables for 1 to 16 days.

### `hourly`

```bash
weather-signal hourly <location> [--hours 48]
```

Returns hourly forecast variables for 1 to 384 hours.

### `signal`

```bash
weather-signal signal <location> [--days 7] [--profile demand]
```

Returns normalized daily weather features for forecasting workflows.

### `summary`

```bash
weather-signal summary <location> [--days 7] [--profile demand]
```

Returns a compact JSON summary over the forecast window, including counts for
risk, warm, hot, wet, windy, and sunny days plus the underlying daily signals.

### `threshold`

```bash
weather-signal threshold <location> [--days 7] --rain-prob-gte 60
weather-signal threshold <location> --precip-mm-gte 5 --wind-gust-gte 40
```

Filters the forecast to days matching one or more threshold conditions.
Supported thresholds are `--rain-prob-gte`, `--precip-mm-gte`,
`--temp-max-gte`, `--temp-min-lte`, and `--wind-gust-gte`.

### `batch signal`

```bash
weather-signal batch signal --places all [--days 7] [--concurrency 4]
weather-signal batch signal --input locations.csv [--country GB] [--concurrency 4]
```

Runs demand signals for multiple locations. `--places all` uses saved places.
CSV input must include a `location` column and may include a `country` column.
Batch output is per-item: each item contains either `signal` or `error`, so one
failed location does not discard successful locations.

### `historical`

```bash
weather-signal historical <location> --start YYYY-MM-DD --end YYYY-MM-DD
```

Returns daily Open-Meteo archive variables for backtesting and feature
engineering.

### `completions`

```bash
weather-signal completions zsh
weather-signal completions bash
weather-signal completions fish
```

Prints shell completion scripts to stdout.

### `server start`

```bash
weather-signal server start --transport stdio
weather-signal server start --transport streamable-http --http-host 127.0.0.1 --http-port 8768 --http-path /mcp
```

Starts the MCP server. `stdio` is intended for local MCP clients. Streamable
HTTP exposes MCP at the configured path and health probes at `/healthz` and
`/readyz`.

### `cache`

```bash
weather-signal cache status
weather-signal cache prune --max-age 7d
weather-signal cache clear
```

Inspects, prunes, or clears local response cache files.
