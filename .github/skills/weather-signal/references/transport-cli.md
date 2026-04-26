# CLI Transport

## Check installation

```bash
weather-signal --version
weather-signal --help
```

If the binary is not installed but the repo is available, use:

```bash
cargo run -- <command>
```

Examples:

```bash
cargo run -- signal london --country GB --days 7
cargo run -- daily london --country GB --days 3 --table
```

## Location discovery

Use geocoding before forecast commands when the location is ambiguous:

```bash
weather-signal geocode "London" --count 5
weather-signal geocode "London" --country GB --count 3
```

Choose the intended result by country/admin area, then use a saved alias if the
location will be reused.

## Saved business locations

```bash
weather-signal places add london "London" --country GB
weather-signal places list --table
weather-signal signal london --days 7
weather-signal places remove london
```

Use saved places for stores, warehouses, regions, and repeatable reporting
locations. Prefer clear aliases such as `store-london-west` over generic names
when multiple business sites exist in one city.

## Demand forecasting signals

Use `signal` when the task is about demand, staffing, inventory, marketing, or
operational planning:

```bash
weather-signal signal london --country GB --days 7
```

For a fresh operational read:

```bash
weather-signal signal london --country GB --days 7 --refresh
```

## Compact agent summary

Use `summary` when the user needs a short overview before deciding whether to
inspect full daily signals:

```bash
weather-signal summary london --country GB --days 7
```

Report the `headline`, date range, cache state, and the largest counts
(`risk_days`, `wet_days`, `warm_days`, `windy_days`, `sunny_days`).

## Threshold screening

Use `threshold` when the task is framed as "which days meet this condition":

```bash
weather-signal threshold london --country GB --days 7 --rain-prob-gte 60
weather-signal threshold london --country GB --precip-mm-gte 5 --wind-gust-gte 40
weather-signal threshold london --country GB --temp-max-gte 25
```

Supported thresholds are `--rain-prob-gte`, `--precip-mm-gte`,
`--temp-max-gte`, `--temp-min-lte`, and `--wind-gust-gte`.

## Batch saved locations

Use batch mode for multi-site forecasting features:

```bash
weather-signal batch signal --places all --days 7 --concurrency 4
weather-signal batch signal --input locations.csv --country GB --days 7 --concurrency 4
```

CSV input must include a `location` column and may include a `country` column.
Each batch item contains either `signal` or `error`; keep successful items even
when one location fails.

## Historical weather

Use `historical` for backtests and feature engineering against known demand:

```bash
weather-signal historical london --country GB --start 2026-04-20 --end 2026-04-25
```

## Daily forecast

Use `daily` when the user needs direct weather variables:

```bash
weather-signal daily london --country GB --days 7
weather-signal daily london --country GB --days 7 --table
weather-signal daily london --country GB --days 7 --output csv
```

## Hourly forecast

Use `hourly` for same-day or next-day operational timing:

```bash
weather-signal hourly london --country GB --hours 24
weather-signal hourly london --country GB --hours 48 --output csv
```

## Current weather

Use `current` for now-context only:

```bash
weather-signal current london --country GB
```

## Cache controls

```bash
weather-signal cache status --table
weather-signal cache prune --max-age 7d
weather-signal cache clear
weather-signal signal london --country GB --refresh
weather-signal signal london --country GB --cache-ttl 10m
```

Default forecast cache TTL is 30 minutes. Geocoding is cached for 30 days
because location metadata changes infrequently. Historical archive responses are
cached for 24 hours.
