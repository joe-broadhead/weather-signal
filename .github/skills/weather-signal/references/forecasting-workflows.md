# Forecasting Workflows

## Role in a demand model

Weather Signal provides weather context and derived features. It does not
forecast business demand by itself. Join its output to demand history, calendar
features, campaigns, promotions, events, and inventory constraints.

## Recommended feature layer

For each business location and date, create features such as:

- `temp_max_c`
- `temp_min_c`
- `apparent_temp_max_c`
- `precip_probability_max_pct`
- `precipitation_mm`
- `wind_gust_max_kmh`
- `sunshine_hours`
- `uv_index_max`
- demand flags from `signal`

Preserve:

- resolved `location`
- `source`
- `fetched_at`
- `cache`
- command used

## Suggested agent pattern

1. Resolve or add saved places for each business location.
2. Run `summary` to quickly understand the weather regime.
3. Run `signal` or `batch signal` for the forecast horizon.
4. Use `threshold` for decision rules such as rain, heat, or wind triggers.
5. Use `historical` to backtest the same features against demand history.
6. Store raw JSON as evidence.
7. Flatten `days[]` to one row per location/date.
8. Join to demand data by location/date.
9. Compare forecast accuracy with and without weather features.

## Example

```bash
weather-signal places add london-store "London" --country GB
weather-signal summary london-store --days 7
weather-signal signal london-store --days 7 --output json
```

Use `--refresh` only for operational decisions that require live API state.
Use `--concurrency` conservatively for batches so upstream rate limits do not
become the dominant failure mode.

## Caveats

- Public Open-Meteo responses may change with forecast model updates.
- Forecast uncertainty generally increases with horizon.
- Location granularity matters: a city-level forecast may be too coarse for
  weather-sensitive last-mile or store-level planning.
- Weather features should be backtested against the business KPI before they
  influence automated decisions.
