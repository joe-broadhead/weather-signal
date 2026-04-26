# Installation

## From Source

```bash
git clone https://github.com/joe-broadhead/weather-signal.git
cd weather-signal
cargo build --release
```

The compiled binary is written to:

```text
target/release/weather-signal
```

For local development, use `cargo run`:

```bash
cargo run -- signal london --country GB --days 7
```

## Rust Version

Weather Signal targets Rust 1.93+ and commits `Cargo.lock` for reproducible
binary builds.

## Open-Meteo Endpoints

By default, the CLI uses the public Open-Meteo APIs:

- Forecast: `https://api.open-meteo.com/v1/forecast`
- Geocoding: `https://geocoding-api.open-meteo.com/v1/search`
- Historical archive: `https://archive-api.open-meteo.com/v1/archive`

For commercial or self-hosted deployments:

```bash
export OPEN_METEO_API_KEY="..."
export OPEN_METEO_FORECAST_BASE_URL="https://customer-api.open-meteo.com/v1/forecast"
export OPEN_METEO_GEOCODING_BASE_URL="https://geocoding-api.open-meteo.com/v1/search"
export OPEN_METEO_HISTORICAL_BASE_URL="https://archive-api.open-meteo.com/v1/archive"
```
