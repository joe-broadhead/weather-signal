# Installation

## Installer

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/weather-signal/HEAD/scripts/install.sh | bash
```

Install the binary and Weather Signal agent skills:

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/weather-signal/HEAD/scripts/install.sh | bash -s -- --install-skills
```

Installer options:

```bash
scripts/install.sh --install-dir "$HOME/.local/bin"
scripts/install.sh --install-skills --skills-dir "$HOME/.agents/skills"
scripts/install.sh --install-skills --skill weather-signal
```

Environment overrides include `WEATHER_SIGNAL_VERSION`,
`WEATHER_SIGNAL_INSTALL_DIR`, `WEATHER_SIGNAL_INSTALL_SKILLS`,
`WEATHER_SIGNAL_SKILLS_DIR`, and `WEATHER_SIGNAL_GITHUB_TOKEN`.

## Prebuilt Binaries

Release assets are published from the GitHub Release workflow with checksums,
SBOMs, and provenance attestations.

```bash
# macOS Apple Silicon
curl -L -o weather-signal.tar.gz \
  https://github.com/joe-broadhead/weather-signal/releases/download/v0.0.1/weather-signal-macos-arm64.tar.gz

tar -xzf weather-signal.tar.gz
./weather-signal-macos-arm64/weather-signal --version
```

Choose the asset for your platform from
[Releases](https://github.com/joe-broadhead/weather-signal/releases).

## From Source

```bash
git clone https://github.com/joe-broadhead/weather-signal.git
cd weather-signal
cargo build --locked --release
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
