# Troubleshooting

## Geocoding Finds the Wrong Place

Add a country hint or use coordinates.

```bash
weather-signal geocode london --country GB --count 5
weather-signal signal "51.5074,-0.1278"
```

For recurring workflows, save the resolved location as an alias:

```bash
weather-signal places add london "London" --country GB
```

## Results Look Stale

Inspect the cache and refresh the request.

```bash
weather-signal cache status
weather-signal signal london --refresh
```

Forecast cache TTL defaults to 30 minutes. Geocoding responses are cached for
30 days, and historical archive responses are cached for 24 hours.

## A Batch Has Partial Failures

`batch signal` is intentionally per-item. Inspect items with an `error` field
and rerun only those locations after fixing the input.

```bash
weather-signal batch signal --input locations.csv --days 7
```

CSV input must include `location` and may include `country`.

## Open-Meteo Returns Rate Limits or Transient Errors

Weather Signal retries bounded transient failures. If failures persist:

- Lower `--concurrency` for batch runs.
- Increase `--timeout` for slow networks.
- Use `--refresh` only when fresh data is required.
- Configure commercial Open-Meteo endpoints when public limits are too low.

## MCP HTTP Does Not Start

Check the transport name and path:

```bash
weather-signal server start --transport streamable-http --http-path /mcp
```

`--http-path` cannot be `/healthz` or `/readyz` because those paths are reserved
for probes.

## Enable Diagnostics

Logs are written to stderr.

```bash
RUST_LOG=weather_signal=debug weather-signal summary london --country GB
```

API keys are redacted from logged URLs.
