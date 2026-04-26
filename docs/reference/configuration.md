# Configuration

## Config File

Default config path:

```text
~/.config/weather-signal/config.toml
```

Override it with:

```bash
weather-signal --config ./config.toml places list
```

Saved places are stored as TOML and can be edited by hand if needed.

## Cache

Default cache path:

```text
~/.cache/weather-signal
```

Forecast cache TTL defaults to 30 minutes and can be overridden per command.
Geocoding responses are cached for 30 days, and historical archive responses are
cached for 24 hours.

```bash
weather-signal signal london --country GB --cache-ttl 10m
```

Use `--refresh` to bypass the cache and write a fresh response. Use
`weather-signal cache prune --max-age 7d` to remove older cache files without
clearing the whole cache.

Network requests use a 30-second timeout by default:

```bash
weather-signal signal london --country GB --timeout 15s
```

## Environment Variables

| Variable | Purpose |
| --- | --- |
| `OPEN_METEO_API_KEY` | Appended to Open-Meteo requests as `apikey` |
| `OPEN_METEO_FORECAST_BASE_URL` | Overrides the forecast endpoint |
| `OPEN_METEO_GEOCODING_BASE_URL` | Overrides the geocoding endpoint |
| `OPEN_METEO_HISTORICAL_BASE_URL` | Overrides the historical archive endpoint |
| `RUST_LOG` | Enables stderr diagnostics, for example `weather_signal=debug` |

Command-line flags take precedence over environment variables.

Logs always go to stderr so JSON, table, and CSV stdout remain parseable.
