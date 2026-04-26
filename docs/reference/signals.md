# Signal Reference

The `signal` command converts daily Open-Meteo forecast variables into a stable
feature payload for demand forecasting workflows.

## Fields

Each day includes:

| Field | Meaning |
| --- | --- |
| `temp_max_c` | Daily max air temperature in Celsius |
| `temp_min_c` | Daily min air temperature in Celsius |
| `apparent_temp_max_c` | Daily max apparent temperature in Celsius |
| `apparent_temp_min_c` | Daily min apparent temperature in Celsius |
| `precipitation_mm` | Total daily precipitation in millimeters |
| `precip_probability_max_pct` | Maximum daily precipitation probability |
| `precipitation_hours` | Hours with precipitation |
| `wind_speed_max_kmh` | Maximum 10m wind speed |
| `wind_gust_max_kmh` | Maximum 10m wind gust |
| `sunshine_hours` | Sunshine duration converted from seconds to hours |
| `uv_index_max` | Maximum daily UV index |
| `weather_code` | Open-Meteo WMO weather code |

## Demand Flags

| Flag | Default rule |
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

These thresholds are intentionally simple in v1. Treat them as stable defaults
for joining weather context into downstream demand models.
