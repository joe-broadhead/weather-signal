# Demand Signals

Use `demand_signal` for demand forecasting feature enrichment.

## Feature priority

| Field | Type | Meaning |
|-------|------|---------|
| `temp_max_c` | float | Daily high temp |
| `temp_min_c` | float | Daily low temp |
| `apparent_temp_max_c` | float | Feels-like high |
| `precip_probability_max_pct` | float | Max rain chance |
| `precipitation_mm` | float | Total precipitation |
| `wind_gust_max_kmh` | float | Max wind gust |
| `sunshine_hours` | float | Hours of sun |
| `uv_index_max` | float | Max UV index |
| `flags.rain_likely` | bool | Rain probability > 50% |
| `flags.wet_day` | bool | Precipitation > 0.5mm |
| `flags.heavy_rain` | bool | Precipitation > 10mm |
| `flags.warm_day` | bool | Max temp > 20°C |
| `flags.hot_day` | bool | Max temp > 25°C |
| `flags.cold_day` | bool | Min temp < 0°C |
| `flags.windy_day` | bool | Max gust > 38 km/h |
| `flags.sunny_day` | bool | Sunshine > 8 hours |
| `flags.high_uv` | bool | UV > 6 |

## Workflow

1. Use `weather_summary` first for compact planning overview.
2. Use `demand_signal` for structured feature data.
3. Flatten `days[]` to one row per location/date.
4. Join weather rows to business demand data by location/date.
5. Compare model behavior with and without weather features.
6. Report weather as explanatory input, not standalone forecast.

## Guardrails

- Do not imply causality from weather features alone.
- Do not collapse multiple locations into one city forecast.
- Include `fetched_at`, `cache`, and resolved `location` metadata in evidence.
- Use `--refresh` for operational decisions; use cache for repeatable analysis.
