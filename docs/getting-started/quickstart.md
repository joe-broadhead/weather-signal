# Quickstart

## Forecast Signals

```bash
weather-signal signal london --country GB --days 3
```

This returns daily demand-oriented features as JSON. JSON is the default output
because the CLI is intended to be called by agents and scripts.

## Human-Readable Output

```bash
weather-signal daily london --country GB --days 3 --table
```

## CSV Output

```bash
weather-signal hourly "51.5072,-0.1276" --hours 24 --output csv
```

## Agent Shortcuts

```bash
weather-signal summary london --country GB --days 7
weather-signal threshold london --country GB --rain-prob-gte 60
weather-signal historical london --country GB --start 2026-04-20 --end 2026-04-25
```

Use `summary` when an agent needs a compact overview, `threshold` when it needs
only days matching decision rules, and `historical` when building or backtesting
weather features.

## Saved Places

Use saved places for recurring business locations:

```bash
weather-signal places add london "London" --country GB
weather-signal places list --table
weather-signal signal london --days 7
```

The resolver checks saved aliases first, then `lat,lon`, then geocoding.

## Cache Control

```bash
weather-signal cache status --table
weather-signal signal london --country GB --refresh
weather-signal cache prune --max-age 7d
weather-signal cache clear
```

Forecast responses are cached for 30 minutes by default. Geocoding responses
are cached for 30 days. Historical archive responses are cached for 24 hours.
