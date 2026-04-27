# Output Contracts

## General response fields

Weather command responses include:

- `source`: currently `open-meteo`
- `location`: resolved location metadata
- `fetched_at`: UTC timestamp for the CLI run
- `cache`: `hit`, `miss`, or `refresh`
- `timezone`: resolved Open-Meteo timezone

Always inspect `location` before acting on output from a place-name query.

## Signal response

`weather-signal signal <location>` returns:

```json
{
  "source": "open-meteo",
  "location": {},
  "fetched_at": "2026-04-26T13:16:30Z",
  "cache": "miss",
  "timezone": "Europe/London",
  "profile": "demand",
  "days": []
}
```

Each `days[]` item includes:

- `date`
- `temp_max_c`
- `temp_min_c`
- `apparent_temp_max_c`
- `apparent_temp_min_c`
- `precipitation_mm`
- `precip_probability_max_pct`
- `precipitation_hours`
- `wind_speed_max_kmh`
- `wind_gust_max_kmh`
- `sunshine_hours`
- `uv_index_max`
- `weather_code`
- `flags`

## Batch signal response

`weather-signal batch signal` returns a per-item result:

```json
{
  "source": "open-meteo",
  "fetched_at": "2026-04-26T13:16:30Z",
  "profile": "demand",
  "items": [
    {
      "input": "London",
      "country": "GB",
      "signal": {}
    },
    {
      "input": "Unknown",
      "error": "no geocoding result found"
    }
  ]
}
```

`country` is the requested/default country hint. The resolved country is in
`signal.location.country_code`. Keep successful items even when another item
contains `error`.

## MCP response handling

MCP tools return JSON text content on success. Tool failures are surfaced as MCP
tool errors (`isError: true`) with contextual error text.

## Demand flags

| Flag | Rule |
| --- | --- |
| `rain_likely` | Precipitation probability max >= 50% |
| `wet_day` | Precipitation sum >= 1 mm |
| `heavy_rain` | Precipitation sum >= 5 mm |
| `warm_day` | Max temperature >= 20 C |
| `hot_day` | Max temperature >= 25 C |
| `cold_day` | Min temperature <= 5 C |
| `windy_day` | Wind gust max >= 40 km/h |
| `sunny_day` | Sunshine duration >= 6 hours |
| `high_uv` | UV index >= 6 |

## Choosing output format

- JSON: default for agents and scripts.
- Table: use when presenting results directly to a human.
- CSV: use for spreadsheets, feature-store imports, or quick joins.

Examples:

```bash
weather-signal signal london --country GB --days 7
weather-signal signal london --country GB --days 7 --table
weather-signal signal london --country GB --days 7 --output csv
```

## Summary standard

When returning a natural-language summary, report the fields that matter for the
task. For demand forecasting, prioritize:

- max/min temperature
- precipitation probability and precipitation amount
- `rain_likely`, `wet_day`, and `heavy_rain`
- `warm_day`, `hot_day`, and `cold_day`
- `windy_day`, `sunny_day`, and `high_uv`
