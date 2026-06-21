# CLI Transport

## Check installation

```bash
weather-signal --version
weather-signal --help
```

## Quick commands

```bash
# Current weather
weather-signal current london --country GB

# Weekly summary
weather-signal summary london --country GB --days 7

# Demand features
weather-signal signal london --country GB --days 7 --profile demand

# Daily forecast
weather-signal daily london --country GB --days 7

# Hourly forecast
weather-signal hourly london --country GB --hours 48

# Threshold filtering
weather-signal threshold london --rain-prob-gte 50 --temp-max-gte 25

# Historical (for backtesting)
weather-signal historical london --country GB --start 2026-01-01 --end 2026-01-07

# Geocode
weather-signal geocode "London" --country GB --count 3

# Cache
weather-signal cache status
```

## Output formats

```bash
weather-signal summary london --country GB
weather-signal summary london --country GB --table
weather-signal summary london --country GB --output csv
```

## Batch processing

```bash
weather-signal batch signal store-london warehouse-birmingham --concurrency 4
```
