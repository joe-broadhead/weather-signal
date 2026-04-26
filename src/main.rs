use anyhow::{Context, Result, anyhow};
use axum::{Json, Router, http::StatusCode, routing::get};
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use comfy_table::{Table, presets::UTF8_FULL};
use futures::{StreamExt, stream};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo, ToolsCapability},
    tool, tool_handler, tool_router,
    transport::{
        StreamableHttpServerConfig, StreamableHttpService,
        streamable_http_server::session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use url::Url;

const APP_NAME: &str = "weather-signal";
const DEFAULT_FORECAST_BASE_URL: &str = "https://api.open-meteo.com/v1/forecast";
const DEFAULT_GEOCODE_BASE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const DEFAULT_HISTORICAL_BASE_URL: &str = "https://archive-api.open-meteo.com/v1/archive";
const SCHEMA_VERSION: &str = "v1";
const DEFAULT_BATCH_CONCURRENCY: usize = 4;
static CACHE_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

const CURRENT_VARS: &[&str] = &[
    "temperature_2m",
    "relative_humidity_2m",
    "apparent_temperature",
    "precipitation",
    "rain",
    "showers",
    "snowfall",
    "weather_code",
    "cloud_cover",
    "wind_speed_10m",
    "wind_gusts_10m",
];

const HOURLY_VARS: &[&str] = &[
    "temperature_2m",
    "apparent_temperature",
    "precipitation_probability",
    "precipitation",
    "rain",
    "weather_code",
    "cloud_cover",
    "wind_speed_10m",
    "wind_gusts_10m",
    "is_day",
];

const DAILY_VARS: &[&str] = &[
    "weather_code",
    "temperature_2m_max",
    "temperature_2m_min",
    "apparent_temperature_max",
    "apparent_temperature_min",
    "precipitation_sum",
    "precipitation_hours",
    "precipitation_probability_max",
    "wind_speed_10m_max",
    "wind_gusts_10m_max",
    "sunshine_duration",
    "uv_index_max",
];

const HISTORICAL_DAILY_VARS: &[&str] = &[
    "weather_code",
    "temperature_2m_max",
    "temperature_2m_min",
    "apparent_temperature_max",
    "apparent_temperature_min",
    "precipitation_sum",
    "rain_sum",
    "precipitation_hours",
    "wind_speed_10m_max",
    "wind_gusts_10m_max",
    "sunshine_duration",
];

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let output = cli.output_format();
    if let Err(err) = run(cli).await {
        if output == OutputFormat::Json {
            eprintln!("{}", json!({"error": err.to_string()}));
        } else {
            eprintln!("error: {err:#}");
        }
        process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    if let Command::Completions(cmd) = &cli.command {
        let mut command = Cli::command();
        generate(cmd.shell, &mut command, APP_NAME, &mut std::io::stdout());
        return Ok(());
    }

    let app = App::new(&cli).await?;
    match &cli.command {
        Command::Geocode(cmd) => {
            let results = app
                .geocode(&cmd.query, cmd.country.as_deref(), cmd.count)
                .await?;
            render(&cli, &results, TableData::Geocode(&results))?;
        }
        Command::Places {
            command: PlacesCommand::Add(cmd),
        } => {
            let place = app
                .resolve_geocoded(&cmd.query, cmd.country.as_deref())
                .await?;
            let mut config = app.config.clone();
            config.places.insert(cmd.alias.clone(), place);
            config.save(&app.config_path)?;
            render(
                &cli,
                &json!({"ok": true}),
                TableData::Message("place saved"),
            )?;
        }
        Command::Places {
            command: PlacesCommand::List,
        } => {
            render(
                &cli,
                &app.config.places,
                TableData::Places(&app.config.places),
            )?;
        }
        Command::Places {
            command: PlacesCommand::Remove(cmd),
        } => {
            let mut config = app.config.clone();
            let removed = config.places.remove(&cmd.alias).is_some();
            config.save(&app.config_path)?;
            render(
                &cli,
                &json!({"removed": removed}),
                TableData::Message(if removed {
                    "place removed"
                } else {
                    "place not found"
                }),
            )?;
        }
        Command::Current(cmd) => {
            let resolved = app
                .resolve_location(&cmd.location, cmd.country.as_deref())
                .await?;
            let envelope = app
                .forecast(&resolved, ForecastKind::Current, 1, None)
                .await?;
            render(&cli, &envelope, TableData::Current(&envelope))?;
        }
        Command::Daily(cmd) => {
            let resolved = app
                .resolve_location(&cmd.location, cmd.country.as_deref())
                .await?;
            let envelope = app
                .forecast(&resolved, ForecastKind::Daily, cmd.days, None)
                .await?;
            render(&cli, &envelope, TableData::Daily(&envelope))?;
        }
        Command::Hourly(cmd) => {
            let resolved = app
                .resolve_location(&cmd.location, cmd.country.as_deref())
                .await?;
            let days = cmd.hours.div_ceil(24).clamp(1, 16) as u8;
            let envelope = app
                .forecast(&resolved, ForecastKind::Hourly, days, Some(cmd.hours))
                .await?;
            render(&cli, &envelope, TableData::Hourly(&envelope))?;
        }
        Command::Signal(cmd) => {
            let signal = app
                .signal_for(&cmd.location, cmd.country.as_deref(), cmd.days, cmd.profile)
                .await?;
            render(&cli, &signal, TableData::Signal(&signal))?;
        }
        Command::Batch {
            command: BatchCommand::Signal(cmd),
        } => {
            let batch = app.batch_signal(cmd).await?;
            render(&cli, &batch, TableData::BatchSignal(&batch))?;
        }
        Command::Threshold(cmd) => {
            let threshold = app.threshold(cmd).await?;
            render(&cli, &threshold, TableData::Threshold(&threshold))?;
        }
        Command::Summary(cmd) => {
            let summary = app.summary(cmd).await?;
            render(&cli, &summary, TableData::Summary(&summary))?;
        }
        Command::Historical(cmd) => {
            let resolved = app
                .resolve_location(&cmd.location, cmd.country.as_deref())
                .await?;
            let start = parse_date(&cmd.start, "start")?;
            let end = parse_date(&cmd.end, "end")?;
            if end < start {
                return Err(anyhow!("end date must be on or after start date"));
            }
            let envelope = app.historical(&resolved, start, end).await?;
            render(&cli, &envelope, TableData::Daily(&envelope))?;
        }
        Command::Server(ServerArgs {
            command: ServerCommand::Start(cmd),
        }) => {
            start_mcp_server(app, cmd).await?;
        }
        Command::Cache {
            command: CacheCommand::Status,
        } => {
            let status = app.cache.status()?;
            render(&cli, &status, TableData::CacheStatus(&status))?;
        }
        Command::Cache {
            command: CacheCommand::Clear,
        } => {
            let removed = app.cache.clear()?;
            render(
                &cli,
                &json!({"removed_files": removed}),
                TableData::Message("cache cleared"),
            )?;
        }
        Command::Cache {
            command: CacheCommand::Prune(cmd),
        } => {
            let removed = app.cache.prune_older_than(cmd.max_age)?;
            render(
                &cli,
                &json!({
                    "removed_files": removed,
                    "max_age": humantime::format_duration(cmd.max_age).to_string()
                }),
                TableData::Message("cache pruned"),
            )?;
        }
        Command::Completions(_) => unreachable!("completions handled before app initialization"),
    }
    Ok(())
}

#[derive(Parser, Debug)]
#[command(name = APP_NAME, version, about = "Agent-first Open-Meteo weather signal CLI")]
struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json, help = "Output format")]
    output: OutputFormat,
    #[arg(
        long,
        global = true,
        conflicts_with = "output",
        help = "Shortcut for --output table"
    )]
    table: bool,
    #[arg(
        long,
        global = true,
        help = "Bypass cache reads and write fresh API responses"
    )]
    refresh: bool,
    #[arg(long, global = true, default_value = "30m", value_parser = parse_duration, help = "Forecast cache TTL, such as 10m or 1h")]
    cache_ttl: Duration,
    #[arg(long, global = true, default_value = "30s", value_parser = parse_duration, help = "HTTP request timeout, such as 15s")]
    timeout: Duration,
    #[arg(
        long,
        global = true,
        env = "OPEN_METEO_API_KEY",
        help = "Open-Meteo API key for commercial endpoints"
    )]
    api_key: Option<String>,
    #[arg(long, global = true, env = "OPEN_METEO_FORECAST_BASE_URL", default_value = DEFAULT_FORECAST_BASE_URL, help = "Open-Meteo forecast endpoint override")]
    forecast_base_url: String,
    #[arg(long, global = true, env = "OPEN_METEO_GEOCODING_BASE_URL", default_value = DEFAULT_GEOCODE_BASE_URL, help = "Open-Meteo geocoding endpoint override")]
    geocode_base_url: String,
    #[arg(long, global = true, env = "OPEN_METEO_HISTORICAL_BASE_URL", default_value = DEFAULT_HISTORICAL_BASE_URL, help = "Open-Meteo historical archive endpoint override")]
    historical_base_url: String,
    #[arg(long, global = true, help = "Path to the saved places TOML config")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    fn output_format(&self) -> OutputFormat {
        if self.table {
            OutputFormat::Table
        } else {
            self.output
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
    Table,
    Csv,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(about = "Search Open-Meteo geocoding for candidate locations")]
    Geocode(GeocodeArgs),
    #[command(about = "Manage saved location aliases")]
    Places {
        #[command(subcommand)]
        command: PlacesCommand,
    },
    #[command(about = "Fetch current weather fields for a location")]
    Current(LocationArgs),
    #[command(about = "Fetch daily forecast variables")]
    Daily(DailyArgs),
    #[command(about = "Fetch hourly forecast variables")]
    Hourly(HourlyArgs),
    #[command(about = "Fetch demand-oriented daily weather signal features")]
    Signal(SignalArgs),
    #[command(about = "Run weather signal workflows for multiple locations")]
    Batch {
        #[command(subcommand)]
        command: BatchCommand,
    },
    #[command(about = "Filter forecast days by decision thresholds")]
    Threshold(ThresholdArgs),
    #[command(about = "Summarize a forecast window for agents")]
    Summary(SummaryArgs),
    #[command(about = "Fetch historical daily weather archive variables")]
    Historical(HistoricalArgs),
    #[command(about = "Run the MCP server")]
    Server(ServerArgs),
    #[command(about = "Inspect or clear the local response cache")]
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
    #[command(about = "Print shell completion scripts")]
    Completions(CompletionsArgs),
}

#[derive(Subcommand, Debug)]
enum PlacesCommand {
    #[command(about = "Resolve and save a location alias")]
    Add(PlaceAddArgs),
    #[command(about = "List saved location aliases")]
    List,
    #[command(about = "Remove a saved location alias")]
    Remove(PlaceRemoveArgs),
}

#[derive(Subcommand, Debug)]
enum CacheCommand {
    #[command(about = "Show local cache path, file count, and bytes")]
    Status,
    #[command(about = "Remove local cache files")]
    Clear,
    #[command(about = "Remove local cache files older than a duration")]
    Prune(CachePruneArgs),
}

#[derive(Subcommand, Debug)]
enum BatchCommand {
    #[command(about = "Run demand signals for saved places or CSV locations")]
    Signal(BatchSignalArgs),
}

#[derive(Args, Debug)]
struct GeocodeArgs {
    #[arg(help = "Place name to search")]
    query: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u16).range(1..=100), help = "Maximum geocoding candidates to return")]
    count: u16,
}

#[derive(Args, Debug)]
struct CachePruneArgs {
    #[arg(
        long,
        value_parser = parse_duration,
        default_value = "7d",
        help = "Remove cache files older than this duration, such as 24h or 7d"
    )]
    max_age: Duration,
}

#[derive(Args, Debug)]
struct PlaceAddArgs {
    #[arg(help = "Alias to save in the local config")]
    alias: String,
    #[arg(help = "Place name or search query to resolve")]
    query: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
}

#[derive(Args, Debug)]
struct PlaceRemoveArgs {
    #[arg(help = "Saved alias to remove")]
    alias: String,
}

#[derive(Args, Debug)]
struct LocationArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
}

#[derive(Args, Debug)]
struct DailyArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    days: u8,
}

#[derive(Args, Debug)]
struct HourlyArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
    #[arg(long, default_value_t = 48, value_parser = clap::value_parser!(u16).range(1..=384), help = "Hourly horizon to return")]
    hours: u16,
}

#[derive(Args, Debug)]
struct SignalArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    days: u8,
    #[arg(long, value_enum, default_value_t = SignalProfile::Demand, help = "Signal derivation profile")]
    profile: SignalProfile,
}

#[derive(Args, Debug)]
struct BatchSignalArgs {
    #[arg(long, value_enum, help = "Saved places source to use")]
    places: Option<BatchPlaces>,
    #[arg(long, help = "CSV with a location column and optional country column")]
    input: Option<PathBuf>,
    #[arg(long, help = "Default ISO 3166 country code for CSV rows")]
    country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    days: u8,
    #[arg(long, value_enum, default_value_t = SignalProfile::Demand, help = "Signal derivation profile")]
    profile: SignalProfile,
    #[arg(long, default_value_t = DEFAULT_BATCH_CONCURRENCY, value_parser = parse_batch_concurrency, help = "Maximum locations to fetch concurrently")]
    concurrency: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BatchPlaces {
    All,
}

#[derive(Args, Debug)]
struct ThresholdArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    days: u8,
    #[arg(
        long,
        help = "Match days with rain probability at or above this percent"
    )]
    rain_prob_gte: Option<i64>,
    #[arg(
        long,
        help = "Match days with precipitation at or above this millimeter total"
    )]
    precip_mm_gte: Option<f64>,
    #[arg(
        long,
        help = "Match days with max temperature at or above this Celsius value"
    )]
    temp_max_gte: Option<f64>,
    #[arg(
        long,
        help = "Match days with min temperature at or below this Celsius value"
    )]
    temp_min_lte: Option<f64>,
    #[arg(long, help = "Match days with wind gust at or above this km/h value")]
    wind_gust_gte: Option<f64>,
}

#[derive(Args, Debug)]
struct SummaryArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    days: u8,
    #[arg(long, value_enum, default_value_t = SignalProfile::Demand, help = "Signal derivation profile")]
    profile: SignalProfile,
}

#[derive(Args, Debug)]
struct HistoricalArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    country: Option<String>,
    #[arg(long, help = "Start date in YYYY-MM-DD format")]
    start: String,
    #[arg(long, help = "End date in YYYY-MM-DD format")]
    end: String,
}

#[derive(Args, Debug)]
struct CompletionsArgs {
    #[arg(help = "Shell to generate completions for")]
    shell: Shell,
}

#[derive(Args, Debug)]
struct ServerArgs {
    #[command(subcommand)]
    command: ServerCommand,
}

#[derive(Subcommand, Debug)]
enum ServerCommand {
    #[command(about = "Start MCP over stdio or streamable HTTP")]
    Start(ServerStartArgs),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ServerTransport {
    Stdio,
    StreamableHttp,
}

#[derive(Args, Debug)]
struct ServerStartArgs {
    #[arg(long, value_enum, default_value_t = ServerTransport::Stdio, help = "MCP transport to serve")]
    transport: ServerTransport,
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "HTTP bind host for streamable HTTP"
    )]
    http_host: String,
    #[arg(
        long,
        default_value_t = 8768,
        help = "HTTP bind port for streamable HTTP"
    )]
    http_port: u16,
    #[arg(long, default_value = "/mcp", help = "HTTP path for the MCP endpoint")]
    http_path: String,
    #[arg(
        long,
        default_value_t = false,
        help = "Enable stateful streamable HTTP sessions"
    )]
    http_stateful_mode: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SignalProfile {
    Demand,
}

impl std::fmt::Display for SignalProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Demand => write!(f, "demand"),
        }
    }
}

fn parse_duration(input: &str) -> std::result::Result<Duration, humantime::DurationError> {
    humantime::parse_duration(input)
}

#[derive(Clone)]
struct App {
    client: reqwest::Client,
    config: Config,
    config_path: PathBuf,
    cache: Cache,
    forecast_base_url: String,
    geocode_base_url: String,
    historical_base_url: String,
    api_key: Option<String>,
    refresh: bool,
    cache_ttl: Duration,
}

impl App {
    async fn new(cli: &Cli) -> Result<Self> {
        let config_path = cli.config.clone().unwrap_or_else(default_config_path);
        let config = Config::load(&config_path)?;
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(cli.timeout)
                .connect_timeout(Duration::from_secs(10))
                .build()
                .context("failed to build HTTP client")?,
            config,
            config_path,
            cache: Cache::new(default_cache_dir())?,
            forecast_base_url: cli.forecast_base_url.clone(),
            geocode_base_url: cli.geocode_base_url.clone(),
            historical_base_url: cli.historical_base_url.clone(),
            api_key: cli.api_key.clone(),
            refresh: cli.refresh,
            cache_ttl: cli.cache_ttl,
        })
    }

    async fn geocode(
        &self,
        query: &str,
        country: Option<&str>,
        count: u16,
    ) -> Result<Vec<Location>> {
        let mut url = Url::parse(&self.geocode_base_url)?;
        url.query_pairs_mut()
            .append_pair("name", query)
            .append_pair("count", &count.to_string())
            .append_pair("language", "en")
            .append_pair("format", "json");
        if let Some(country) = country {
            url.query_pairs_mut().append_pair("countryCode", country);
        }
        self.append_api_key(&mut url);

        let response: GeocodeResponse = self
            .get_cached(url.as_str(), Duration::from_secs(30 * 24 * 60 * 60))
            .await?
            .value;
        Ok(response.results.unwrap_or_default())
    }

    async fn resolve_geocoded(&self, query: &str, country: Option<&str>) -> Result<Location> {
        self.geocode(query, country, 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no geocoding result for {query:?}"))
    }

    async fn resolve_location(&self, input: &str, country: Option<&str>) -> Result<Location> {
        if let Some(place) = self.config.places.get(input) {
            return Ok(place.clone());
        }
        if let Some((lat, lon)) = parse_lat_lon(input) {
            validate_coordinates(lat, lon)?;
            return Ok(Location {
                id: None,
                name: input.to_string(),
                country: None,
                country_code: None,
                admin1: None,
                latitude: lat,
                longitude: lon,
                timezone: None,
                population: None,
            });
        }
        self.resolve_geocoded(input, country).await
    }

    async fn forecast(
        &self,
        location: &Location,
        kind: ForecastKind,
        days: u8,
        hourly_limit: Option<u16>,
    ) -> Result<ForecastEnvelope> {
        let mut url = Url::parse(&self.forecast_base_url)?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("latitude", &location.latitude.to_string())
                .append_pair("longitude", &location.longitude.to_string())
                .append_pair("forecast_days", &days.to_string())
                .append_pair("timezone", "auto");
            match kind {
                ForecastKind::Current => {
                    query.append_pair("current", &CURRENT_VARS.join(","));
                }
                ForecastKind::Daily => {
                    query.append_pair("daily", &DAILY_VARS.join(","));
                }
                ForecastKind::Hourly => {
                    query.append_pair("hourly", &HOURLY_VARS.join(","));
                }
                ForecastKind::Signal => {
                    query.append_pair("daily", &DAILY_VARS.join(","));
                }
            }
        }
        self.append_api_key(&mut url);

        let cached: Cached<ForecastResponse> =
            self.get_cached(url.as_str(), self.cache_ttl).await?;
        let mut response = cached.value;
        if let (Some(hours), Some(hourly)) = (hourly_limit, response.hourly.as_mut()) {
            hourly.truncate(usize::from(hours));
        }

        Ok(ForecastEnvelope {
            source: "open-meteo".to_string(),
            location: location.clone(),
            fetched_at: Utc::now(),
            cache: cached.state,
            timezone: response.timezone,
            current: response.current,
            hourly: response.hourly,
            daily: response.daily,
        })
    }

    async fn historical(
        &self,
        location: &Location,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<ForecastEnvelope> {
        let mut url = Url::parse(&self.historical_base_url)?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("latitude", &location.latitude.to_string())
                .append_pair("longitude", &location.longitude.to_string())
                .append_pair("start_date", &start.to_string())
                .append_pair("end_date", &end.to_string())
                .append_pair("daily", &HISTORICAL_DAILY_VARS.join(","))
                .append_pair("timezone", "auto");
        }
        self.append_api_key(&mut url);

        let cached: Cached<ForecastResponse> = self
            .get_cached(url.as_str(), Duration::from_secs(24 * 60 * 60))
            .await?;
        let response = cached.value;
        Ok(ForecastEnvelope {
            source: "open-meteo-archive".to_string(),
            location: location.clone(),
            fetched_at: Utc::now(),
            cache: cached.state,
            timezone: response.timezone,
            current: None,
            hourly: None,
            daily: response.daily,
        })
    }

    async fn signal_for(
        &self,
        location: &str,
        country: Option<&str>,
        days: u8,
        profile: SignalProfile,
    ) -> Result<SignalEnvelope> {
        let resolved = self.resolve_location(location, country).await?;
        let envelope = self
            .forecast(&resolved, ForecastKind::Signal, days, None)
            .await?;
        SignalEnvelope::from_forecast(envelope, profile)
    }

    async fn batch_signal(&self, args: &BatchSignalArgs) -> Result<BatchSignalEnvelope> {
        let inputs = self.batch_location_inputs(args)?;
        let app = self.clone();
        let items = stream::iter(inputs.into_iter().map(|input| {
            let app = app.clone();
            async move { app.batch_signal_item(input, args.days, args.profile).await }
        }))
        .buffer_unordered(args.concurrency)
        .collect()
        .await;
        Ok(BatchSignalEnvelope {
            source: "open-meteo".to_string(),
            fetched_at: Utc::now(),
            profile: args.profile.to_string(),
            items,
        })
    }

    async fn batch_signal_item(
        &self,
        input: BatchLocationInput,
        days: u8,
        profile: SignalProfile,
    ) -> BatchSignalItem {
        let result = async {
            let location = self
                .resolve_location(&input.location, input.country.as_deref())
                .await?;
            let envelope = self
                .forecast(&location, ForecastKind::Signal, days, None)
                .await?;
            SignalEnvelope::from_forecast(envelope, profile)
        }
        .await;

        match result {
            Ok(signal) => BatchSignalItem {
                input: input.location,
                country: input.country,
                signal: Some(signal),
                error: None,
            },
            Err(error) => BatchSignalItem {
                input: input.location,
                country: input.country,
                signal: None,
                error: Some(error.to_string()),
            },
        }
    }

    fn batch_location_inputs(&self, args: &BatchSignalArgs) -> Result<Vec<BatchLocationInput>> {
        match (args.places, args.input.as_ref()) {
            (Some(BatchPlaces::All), None) => {
                if self.config.places.is_empty() {
                    return Err(anyhow!("no saved places found"));
                }
                Ok(self
                    .config
                    .places
                    .keys()
                    .map(|alias| BatchLocationInput {
                        location: alias.clone(),
                        country: None,
                    })
                    .collect())
            }
            (None, Some(path)) => self.batch_locations_from_csv(path, args.country.as_deref()),
            (None, None) => Err(anyhow!(
                "batch signal requires --places all or --input <csv>"
            )),
            (Some(_), Some(_)) => Err(anyhow!("use either --places all or --input, not both")),
        }
    }

    fn batch_locations_from_csv(
        &self,
        path: &Path,
        default_country: Option<&str>,
    ) -> Result<Vec<BatchLocationInput>> {
        let mut reader = csv::Reader::from_path(path)
            .with_context(|| format!("failed to read batch input {}", path.display()))?;
        let mut locations = Vec::new();
        for record in reader.deserialize::<BatchLocationRecord>() {
            let record = record.with_context(|| {
                format!("failed to parse batch input record in {}", path.display())
            })?;
            locations.push(BatchLocationInput {
                location: record.location,
                country: record
                    .country
                    .or_else(|| default_country.map(str::to_string)),
            });
        }
        if locations.is_empty() {
            return Err(anyhow!("batch input has no locations"));
        }
        Ok(locations)
    }

    async fn threshold(&self, args: &ThresholdArgs) -> Result<ThresholdEnvelope> {
        let criteria = ThresholdCriteria::from_args(args)?;
        let signal = self
            .signal_for(
                &args.location,
                args.country.as_deref(),
                args.days,
                SignalProfile::Demand,
            )
            .await?;
        let matches = signal
            .days
            .iter()
            .filter_map(|day| {
                let reasons = criteria.match_reasons(day);
                (!reasons.is_empty()).then(|| ThresholdMatch {
                    date: day.date.clone(),
                    reasons,
                    signal: day.clone(),
                })
            })
            .collect();
        Ok(ThresholdEnvelope {
            source: signal.source,
            location: signal.location,
            fetched_at: signal.fetched_at,
            cache: signal.cache,
            timezone: signal.timezone,
            criteria,
            matches,
        })
    }

    async fn summary(&self, args: &SummaryArgs) -> Result<SummaryEnvelope> {
        let signal = self
            .signal_for(
                &args.location,
                args.country.as_deref(),
                args.days,
                args.profile,
            )
            .await?;
        Ok(SummaryEnvelope::from_signal(signal))
    }

    async fn get_cached<T: DeserializeOwned>(&self, url: &str, ttl: Duration) -> Result<Cached<T>> {
        if !self.refresh
            && let Some(value) = self.cache.get(url, ttl)?
        {
            let value =
                serde_json::from_value(value).context("failed to decode cached response")?;
            return Ok(Cached {
                value,
                state: CacheState::Hit,
            });
        }
        let value: Value = self
            .client
            .get(url)
            .send()
            .await
            .context("request failed")?
            .error_for_status()
            .context("API returned an error status")?
            .json()
            .await
            .context("failed to decode API response")?;
        self.cache.put(url, &value)?;
        let state = if self.refresh {
            CacheState::Refresh
        } else {
            CacheState::Miss
        };
        let value = serde_json::from_value(value).context("failed to decode API response")?;
        Ok(Cached { value, state })
    }

    fn append_api_key(&self, url: &mut Url) {
        if let Some(key) = self.api_key.as_deref().filter(|key| !key.is_empty()) {
            url.query_pairs_mut().append_pair("apikey", key);
        }
    }
}

#[derive(Clone)]
struct WeatherMcpServer {
    app: App,
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for WeatherMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability::default()),
                ..ServerCapabilities::default()
            },
            server_info: Implementation {
                name: APP_NAME.into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Implementation::default()
            },
            instructions: Some("Weather Signal MCP server. Use weather_summary for a compact forecast-window overview, demand_signal for daily demand features, threshold_days for decision rules, and historical_weather for backtesting features.".into()),
        }
    }
}

impl WeatherMcpServer {
    fn new(app: App) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    fn success<T: Serialize>(value: &T) -> String {
        serde_json::to_string(value).unwrap_or_else(|err| {
            json!({
                "success": false,
                "error": format!("serialization failed: {err}")
            })
            .to_string()
        })
    }

    fn error(error: anyhow::Error) -> String {
        json!({
            "success": false,
            "error": error.to_string()
        })
        .to_string()
    }

    async fn handle<T, Fut>(&self, future: Fut) -> String
    where
        T: Serialize,
        Fut: std::future::Future<Output = Result<T>>,
    {
        match future.await {
            Ok(value) => Self::success(&value),
            Err(error) => Self::error(error),
        }
    }
}

#[tool_router]
impl WeatherMcpServer {
    #[tool(
        name = "geocode",
        description = "Resolve a place name using Open-Meteo geocoding. Use before weather tools when location names are ambiguous."
    )]
    async fn geocode_tool(&self, params: Parameters<GeocodeToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            let count = params.count.unwrap_or(5);
            if !(1..=100).contains(&count) {
                return Err(anyhow!("count must be between 1 and 100"));
            }
            self.app
                .geocode(&params.query, params.country.as_deref(), count)
                .await
        })
        .await
    }

    #[tool(
        name = "current_weather",
        description = "Fetch current Open-Meteo weather fields for a resolved location, saved alias, or lat,lon."
    )]
    async fn current_weather(&self, params: Parameters<LocationToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            let resolved = self
                .app
                .resolve_location(&params.location, params.country.as_deref())
                .await?;
            self.app
                .forecast(&resolved, ForecastKind::Current, 1, None)
                .await
        })
        .await
    }

    #[tool(
        name = "daily_forecast",
        description = "Fetch daily Open-Meteo forecast variables for 1 to 16 days."
    )]
    async fn daily_forecast(&self, params: Parameters<DaysToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            let days = normalize_days(params.days)?;
            let resolved = self
                .app
                .resolve_location(&params.location, params.country.as_deref())
                .await?;
            self.app
                .forecast(&resolved, ForecastKind::Daily, days, None)
                .await
        })
        .await
    }

    #[tool(
        name = "hourly_forecast",
        description = "Fetch hourly Open-Meteo forecast variables for 1 to 384 hours."
    )]
    async fn hourly_forecast(&self, params: Parameters<HourlyToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            let hours = params.hours.unwrap_or(48);
            if !(1..=384).contains(&hours) {
                return Err(anyhow!("hours must be between 1 and 384"));
            }
            let days = hours.div_ceil(24).clamp(1, 16) as u8;
            let resolved = self
                .app
                .resolve_location(&params.location, params.country.as_deref())
                .await?;
            self.app
                .forecast(&resolved, ForecastKind::Hourly, days, Some(hours))
                .await
        })
        .await
    }

    #[tool(
        name = "demand_signal",
        description = "Return normalized daily demand-oriented weather features and flags for a forecast window."
    )]
    async fn demand_signal(&self, params: Parameters<DaysToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            self.app
                .signal_for(
                    &params.location,
                    params.country.as_deref(),
                    normalize_days(params.days)?,
                    SignalProfile::Demand,
                )
                .await
        })
        .await
    }

    #[tool(
        name = "weather_summary",
        description = "Return a compact forecast-window summary with risk, warm, hot, wet, windy, and sunny day counts."
    )]
    async fn weather_summary(&self, params: Parameters<DaysToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            self.app
                .summary(&SummaryArgs {
                    location: params.location,
                    country: params.country,
                    days: normalize_days(params.days)?,
                    profile: SignalProfile::Demand,
                })
                .await
        })
        .await
    }

    #[tool(
        name = "threshold_days",
        description = "Filter forecast days by decision thresholds such as rain probability, precipitation, temperature, or wind gust."
    )]
    async fn threshold_days(&self, params: Parameters<ThresholdToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            self.app
                .threshold(&ThresholdArgs {
                    location: params.location,
                    country: params.country,
                    days: normalize_days(params.days)?,
                    rain_prob_gte: params.rain_prob_gte,
                    precip_mm_gte: params.precip_mm_gte,
                    temp_max_gte: params.temp_max_gte,
                    temp_min_lte: params.temp_min_lte,
                    wind_gust_gte: params.wind_gust_gte,
                })
                .await
        })
        .await
    }

    #[tool(
        name = "historical_weather",
        description = "Fetch daily Open-Meteo archive variables for a date range, useful for backtesting weather features."
    )]
    async fn historical_weather(&self, params: Parameters<HistoricalToolParams>) -> String {
        let params = params.0;
        self.handle(async move {
            let start = parse_date(&params.start, "start")?;
            let end = parse_date(&params.end, "end")?;
            if end < start {
                return Err(anyhow!("end date must be on or after start date"));
            }
            let resolved = self
                .app
                .resolve_location(&params.location, params.country.as_deref())
                .await?;
            self.app.historical(&resolved, start, end).await
        })
        .await
    }

    #[tool(
        name = "list_places",
        description = "List saved weather location aliases."
    )]
    async fn list_places(&self) -> String {
        Self::success(&self.app.config.places)
    }

    #[tool(
        name = "cache_status",
        description = "Inspect the local weather response cache path, file count, and byte size."
    )]
    async fn cache_status(&self) -> String {
        match self.app.cache.status() {
            Ok(status) => Self::success(&status),
            Err(error) => Self::error(error),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GeocodeToolParams {
    query: String,
    country: Option<String>,
    count: Option<u16>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LocationToolParams {
    location: String,
    country: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DaysToolParams {
    location: String,
    country: Option<String>,
    days: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HourlyToolParams {
    location: String,
    country: Option<String>,
    hours: Option<u16>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ThresholdToolParams {
    location: String,
    country: Option<String>,
    days: Option<u8>,
    rain_prob_gte: Option<i64>,
    precip_mm_gte: Option<f64>,
    temp_max_gte: Option<f64>,
    temp_min_lte: Option<f64>,
    wind_gust_gte: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HistoricalToolParams {
    location: String,
    country: Option<String>,
    start: String,
    end: String,
}

fn normalize_days(days: Option<u8>) -> Result<u8> {
    let days = days.unwrap_or(7);
    if !(1..=16).contains(&days) {
        return Err(anyhow!("days must be between 1 and 16"));
    }
    Ok(days)
}

fn parse_batch_concurrency(input: &str) -> Result<usize, String> {
    let value = input
        .parse::<usize>()
        .map_err(|_| "concurrency must be an integer between 1 and 32".to_string())?;
    if !(1..=32).contains(&value) {
        return Err("concurrency must be between 1 and 32".to_string());
    }
    Ok(value)
}

async fn start_mcp_server(app: App, args: &ServerStartArgs) -> Result<()> {
    let shutdown = CancellationToken::new();
    spawn_shutdown(shutdown.clone());
    let server = WeatherMcpServer::new(app);
    match args.transport {
        ServerTransport::Stdio => serve_mcp_stdio(server, shutdown).await,
        ServerTransport::StreamableHttp => serve_mcp_http(server, args, shutdown).await,
    }
}

fn spawn_shutdown(shutdown: CancellationToken) {
    tokio::spawn(async move {
        let _ = wait_for_shutdown_signal().await;
        shutdown.cancel();
    });
}

async fn serve_mcp_stdio(server: WeatherMcpServer, shutdown: CancellationToken) -> Result<()> {
    let transport = rmcp::transport::io::stdio();
    let running = rmcp::serve_server(server, transport)
        .await
        .map_err(|error| anyhow!("MCP stdio server failed: {error}"))?;
    let cancel = running.cancellation_token();
    tokio::spawn(async move {
        shutdown.cancelled().await;
        cancel.cancel();
    });
    running
        .waiting()
        .await
        .map_err(|error| anyhow!("MCP stdio server stopped with error: {error}"))?;
    Ok(())
}

async fn serve_mcp_http(
    server: WeatherMcpServer,
    args: &ServerStartArgs,
    shutdown: CancellationToken,
) -> Result<()> {
    warn_if_public_http_bind(&args.http_host, args.http_port, &args.http_path);
    let service: StreamableHttpService<WeatherMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok::<WeatherMcpServer, io::Error>(server.clone()),
            Arc::default(),
            StreamableHttpServerConfig {
                stateful_mode: args.http_stateful_mode,
                sse_keep_alive: Some(Duration::from_secs(30)),
                sse_retry: Some(Duration::from_secs(5)),
                cancellation_token: shutdown.clone(),
            },
        );

    let base_app = Router::new()
        .route("/healthz", get(http_liveness))
        .route("/readyz", get(http_readiness));
    let path = normalized_http_path(&args.http_path);
    let app = if path == "/" {
        base_app.fallback_service(service)
    } else {
        base_app.nest_service(path.as_str(), service)
    };
    let listener = TcpListener::bind((args.http_host.as_str(), args.http_port))
        .await
        .with_context(|| format!("failed to bind {}:{}", args.http_host, args.http_port))?;
    let local_addr = listener.local_addr().context("failed to read local addr")?;
    eprintln!("weather-signal MCP HTTP listening on http://{local_addr}{path}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.cancelled_owned().await;
        })
        .await
        .context("MCP HTTP server failed")?;
    Ok(())
}

fn warn_if_public_http_bind(host: &str, port: u16, path: &str) {
    if !is_loopback_host(host) {
        eprintln!(
            "warning: MCP HTTP has no built-in authentication; protect http://{host}:{port}{} with an authenticating proxy before exposing it",
            normalized_http_path(path)
        );
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host.trim(), "127.0.0.1" | "localhost" | "::1")
}

async fn http_liveness() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
        })),
    )
}

async fn http_readiness() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ready",
            "version": env!("CARGO_PKG_VERSION"),
        })),
    )
}

fn normalized_http_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    format!("/{}", trimmed.trim_matches('/'))
}

async fn wait_for_shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term =
            signal(SignalKind::terminate()).context("failed to install SIGTERM handler")?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.context("failed to listen for SIGINT")?;
            }
            _ = term.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .context("failed to listen for Ctrl-C")?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum ForecastKind {
    Current,
    Daily,
    Hourly,
    Signal,
}

struct Cached<T> {
    value: T,
    state: CacheState,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    places: BTreeMap<String, Location>,
}

impl Config {
    fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("failed to parse config {}", path.display()))
    }

    fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text).with_context(|| format!("failed to write config {}", path.display()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Location {
    id: Option<i64>,
    name: String,
    country: Option<String>,
    country_code: Option<String>,
    admin1: Option<String>,
    latitude: f64,
    longitude: f64,
    timezone: Option<String>,
    population: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GeocodeResponse {
    results: Option<Vec<Location>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CacheState {
    Hit,
    Miss,
    Refresh,
}

impl std::fmt::Display for CacheState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hit => write!(f, "hit"),
            Self::Miss => write!(f, "miss"),
            Self::Refresh => write!(f, "refresh"),
        }
    }
}

#[derive(Clone)]
struct Cache {
    dir: PathBuf,
}

impl Cache {
    fn new(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn key(&self, url: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(SCHEMA_VERSION.as_bytes());
        hasher.update(url.as_bytes());
        let digest = hex::encode(hasher.finalize());
        self.dir.join(format!("{digest}.json"))
    }

    fn get(&self, url: &str, ttl: Duration) -> Result<Option<Value>> {
        let path = self.key(url);
        if !path.exists() {
            return Ok(None);
        }
        let modified = fs::metadata(&path)?.modified()?;
        if modified.elapsed().unwrap_or(Duration::MAX) > ttl {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }

    fn put(&self, url: &str, value: &Value) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.key(url);
        let temp_path = self.temp_key(&path);
        fs::write(&temp_path, serde_json::to_vec(value)?)?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::rename(&temp_path, path).inspect_err(|_rename_error| {
            let _ = fs::remove_file(&temp_path);
        })?;
        Ok(())
    }

    fn temp_key(&self, path: &Path) -> PathBuf {
        let counter = CACHE_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.with_extension(format!("json.tmp.{}.{}", process::id(), counter))
    }

    fn status(&self) -> Result<CacheStatus> {
        let mut files = 0_u64;
        let mut bytes = 0_u64;
        if self.dir.exists() {
            for entry in fs::read_dir(&self.dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    files += 1;
                    bytes += entry.metadata()?.len();
                }
            }
        }
        Ok(CacheStatus {
            path: self.dir.clone(),
            files,
            bytes,
        })
    }

    fn clear(&self) -> Result<u64> {
        let mut removed = 0;
        if self.dir.exists() {
            for entry in fs::read_dir(&self.dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    fs::remove_file(entry.path())?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    fn prune_older_than(&self, max_age: Duration) -> Result<u64> {
        let mut removed = 0;
        if self.dir.exists() {
            for entry in fs::read_dir(&self.dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let modified = entry.metadata()?.modified()?;
                if modified.elapsed().unwrap_or(Duration::MAX) > max_age {
                    fs::remove_file(entry.path())?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

#[derive(Serialize)]
struct CacheStatus {
    path: PathBuf,
    files: u64,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
struct ForecastResponse {
    timezone: String,
    current: Option<BTreeMap<String, Value>>,
    hourly: Option<Series>,
    daily: Option<Series>,
}

#[derive(Debug, Serialize)]
struct ForecastEnvelope {
    source: String,
    location: Location,
    fetched_at: DateTime<Utc>,
    cache: CacheState,
    timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hourly: Option<Series>,
    #[serde(skip_serializing_if = "Option::is_none")]
    daily: Option<Series>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Series {
    time: Vec<String>,
    #[serde(flatten)]
    values: BTreeMap<String, Vec<Value>>,
}

impl Series {
    fn truncate(&mut self, limit: usize) {
        self.time.truncate(limit);
        for values in self.values.values_mut() {
            values.truncate(limit);
        }
    }

    fn len(&self) -> usize {
        self.time.len()
    }

    fn get_f64(&self, key: &str, idx: usize) -> Option<f64> {
        self.values.get(key)?.get(idx)?.as_f64()
    }

    fn get_i64(&self, key: &str, idx: usize) -> Option<i64> {
        self.values.get(key)?.get(idx)?.as_i64()
    }
}

#[derive(Debug, Serialize)]
struct SignalEnvelope {
    source: String,
    location: Location,
    fetched_at: DateTime<Utc>,
    cache: CacheState,
    timezone: String,
    profile: String,
    days: Vec<DailySignal>,
}

#[derive(Clone, Debug, Serialize)]
struct DailySignal {
    date: String,
    temp_max_c: Option<f64>,
    temp_min_c: Option<f64>,
    apparent_temp_max_c: Option<f64>,
    apparent_temp_min_c: Option<f64>,
    precipitation_mm: Option<f64>,
    precip_probability_max_pct: Option<i64>,
    precipitation_hours: Option<f64>,
    wind_speed_max_kmh: Option<f64>,
    wind_gust_max_kmh: Option<f64>,
    sunshine_hours: Option<f64>,
    uv_index_max: Option<f64>,
    weather_code: Option<i64>,
    flags: DemandFlags,
}

#[derive(Clone, Debug, Serialize)]
struct DemandFlags {
    rain_likely: bool,
    wet_day: bool,
    heavy_rain: bool,
    warm_day: bool,
    hot_day: bool,
    cold_day: bool,
    windy_day: bool,
    sunny_day: bool,
    high_uv: bool,
}

#[derive(Debug, Deserialize)]
struct BatchLocationRecord {
    location: String,
    #[serde(default)]
    country: Option<String>,
}

#[derive(Clone, Debug)]
struct BatchLocationInput {
    location: String,
    country: Option<String>,
}

#[derive(Debug, Serialize)]
struct BatchSignalEnvelope {
    source: String,
    fetched_at: DateTime<Utc>,
    profile: String,
    items: Vec<BatchSignalItem>,
}

#[derive(Debug, Serialize)]
struct BatchSignalItem {
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signal: Option<SignalEnvelope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ThresholdEnvelope {
    source: String,
    location: Location,
    fetched_at: DateTime<Utc>,
    cache: CacheState,
    timezone: String,
    criteria: ThresholdCriteria,
    matches: Vec<ThresholdMatch>,
}

#[derive(Clone, Debug, Serialize)]
struct ThresholdCriteria {
    #[serde(skip_serializing_if = "Option::is_none")]
    rain_prob_gte: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precip_mm_gte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temp_max_gte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temp_min_lte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_gust_gte: Option<f64>,
}

impl ThresholdCriteria {
    fn from_args(args: &ThresholdArgs) -> Result<Self> {
        let criteria = Self {
            rain_prob_gte: args.rain_prob_gte,
            precip_mm_gte: args.precip_mm_gte,
            temp_max_gte: args.temp_max_gte,
            temp_min_lte: args.temp_min_lte,
            wind_gust_gte: args.wind_gust_gte,
        };
        if criteria.is_empty() {
            return Err(anyhow!("provide at least one threshold option"));
        }
        Ok(criteria)
    }

    fn is_empty(&self) -> bool {
        self.rain_prob_gte.is_none()
            && self.precip_mm_gte.is_none()
            && self.temp_max_gte.is_none()
            && self.temp_min_lte.is_none()
            && self.wind_gust_gte.is_none()
    }

    fn match_reasons(&self, day: &DailySignal) -> Vec<String> {
        let mut reasons = Vec::new();
        if self.rain_prob_gte.is_some_and(|threshold| {
            day.precip_probability_max_pct
                .is_some_and(|v| v >= threshold)
        }) {
            reasons.push("rain_prob_gte".to_string());
        }
        if self
            .precip_mm_gte
            .is_some_and(|threshold| day.precipitation_mm.is_some_and(|v| v >= threshold))
        {
            reasons.push("precip_mm_gte".to_string());
        }
        if self
            .temp_max_gte
            .is_some_and(|threshold| day.temp_max_c.is_some_and(|v| v >= threshold))
        {
            reasons.push("temp_max_gte".to_string());
        }
        if self
            .temp_min_lte
            .is_some_and(|threshold| day.temp_min_c.is_some_and(|v| v <= threshold))
        {
            reasons.push("temp_min_lte".to_string());
        }
        if self
            .wind_gust_gte
            .is_some_and(|threshold| day.wind_gust_max_kmh.is_some_and(|v| v >= threshold))
        {
            reasons.push("wind_gust_gte".to_string());
        }
        reasons
    }
}

#[derive(Debug, Serialize)]
struct ThresholdMatch {
    date: String,
    reasons: Vec<String>,
    signal: DailySignal,
}

#[derive(Debug, Serialize)]
struct SummaryEnvelope {
    source: String,
    location: Location,
    fetched_at: DateTime<Utc>,
    cache: CacheState,
    timezone: String,
    profile: String,
    headline: String,
    total_days: usize,
    risk_days: usize,
    warm_days: usize,
    hot_days: usize,
    wet_days: usize,
    windy_days: usize,
    sunny_days: usize,
    days: Vec<DailySignal>,
}

impl SummaryEnvelope {
    fn from_signal(signal: SignalEnvelope) -> Self {
        let risk_days = signal
            .days
            .iter()
            .filter(|day| {
                day.flags.rain_likely
                    || day.flags.wet_day
                    || day.flags.heavy_rain
                    || day.flags.windy_day
                    || day.flags.high_uv
            })
            .count();
        let warm_days = signal.days.iter().filter(|day| day.flags.warm_day).count();
        let hot_days = signal.days.iter().filter(|day| day.flags.hot_day).count();
        let wet_days = signal.days.iter().filter(|day| day.flags.wet_day).count();
        let windy_days = signal.days.iter().filter(|day| day.flags.windy_day).count();
        let sunny_days = signal.days.iter().filter(|day| day.flags.sunny_day).count();
        let total_days = signal.days.len();
        let headline = format!(
            "{risk_days} risk days, {warm_days} warm days, {wet_days} wet days, {windy_days} windy days over {total_days} days"
        );
        Self {
            source: signal.source,
            location: signal.location,
            fetched_at: signal.fetched_at,
            cache: signal.cache,
            timezone: signal.timezone,
            profile: signal.profile,
            headline,
            total_days,
            risk_days,
            warm_days,
            hot_days,
            wet_days,
            windy_days,
            sunny_days,
            days: signal.days,
        }
    }
}

impl SignalEnvelope {
    fn from_forecast(envelope: ForecastEnvelope, profile: SignalProfile) -> Result<Self> {
        let daily = envelope
            .daily
            .as_ref()
            .ok_or_else(|| anyhow!("daily data missing"))?;
        let mut days = Vec::with_capacity(daily.len());
        for idx in 0..daily.len() {
            let precip_probability = daily.get_i64("precipitation_probability_max", idx);
            let precipitation = daily.get_f64("precipitation_sum", idx);
            let temp_max = daily.get_f64("temperature_2m_max", idx);
            let temp_min = daily.get_f64("temperature_2m_min", idx);
            let wind_gust = daily.get_f64("wind_gusts_10m_max", idx);
            let sunshine_hours = daily.get_f64("sunshine_duration", idx).map(|v| v / 3600.0);
            let uv = daily.get_f64("uv_index_max", idx);
            let flags = DemandFlags {
                rain_likely: precip_probability.is_some_and(|v| v >= 50),
                wet_day: precipitation.is_some_and(|v| v >= 1.0),
                heavy_rain: precipitation.is_some_and(|v| v >= 5.0),
                warm_day: temp_max.is_some_and(|v| v >= 20.0),
                hot_day: temp_max.is_some_and(|v| v >= 25.0),
                cold_day: temp_min.is_some_and(|v| v <= 5.0),
                windy_day: wind_gust.is_some_and(|v| v >= 40.0),
                sunny_day: sunshine_hours.is_some_and(|v| v >= 6.0),
                high_uv: uv.is_some_and(|v| v >= 6.0),
            };
            days.push(DailySignal {
                date: daily.time[idx].clone(),
                temp_max_c: temp_max,
                temp_min_c: temp_min,
                apparent_temp_max_c: daily.get_f64("apparent_temperature_max", idx),
                apparent_temp_min_c: daily.get_f64("apparent_temperature_min", idx),
                precipitation_mm: precipitation,
                precip_probability_max_pct: precip_probability,
                precipitation_hours: daily.get_f64("precipitation_hours", idx),
                wind_speed_max_kmh: daily.get_f64("wind_speed_10m_max", idx),
                wind_gust_max_kmh: wind_gust,
                sunshine_hours,
                uv_index_max: uv,
                weather_code: daily.get_i64("weather_code", idx),
                flags,
            });
        }
        Ok(Self {
            source: envelope.source,
            location: envelope.location,
            fetched_at: envelope.fetched_at,
            cache: envelope.cache,
            timezone: envelope.timezone,
            profile: profile.to_string(),
            days,
        })
    }
}

enum TableData<'a> {
    Geocode(&'a [Location]),
    Places(&'a BTreeMap<String, Location>),
    Current(&'a ForecastEnvelope),
    Daily(&'a ForecastEnvelope),
    Hourly(&'a ForecastEnvelope),
    Signal(&'a SignalEnvelope),
    BatchSignal(&'a BatchSignalEnvelope),
    Threshold(&'a ThresholdEnvelope),
    Summary(&'a SummaryEnvelope),
    CacheStatus(&'a CacheStatus),
    Message(&'a str),
}

fn render<T: Serialize>(cli: &Cli, value: &T, table_data: TableData<'_>) -> Result<()> {
    match cli.output_format() {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(value)?),
        OutputFormat::Table => println!("{}", to_table(table_data)?),
        OutputFormat::Csv => to_csv(table_data)?,
    }
    Ok(())
}

fn to_table(data: TableData<'_>) -> Result<String> {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    match data {
        TableData::Geocode(locations) => {
            table.set_header(["Name", "Country", "Admin", "Lat", "Lon", "Timezone"]);
            for location in locations {
                table.add_row([
                    location.name.clone(),
                    location.country_code.clone().unwrap_or_default(),
                    location.admin1.clone().unwrap_or_default(),
                    location.latitude.to_string(),
                    location.longitude.to_string(),
                    location.timezone.clone().unwrap_or_default(),
                ]);
            }
        }
        TableData::Places(places) => {
            table.set_header(["Alias", "Name", "Country", "Lat", "Lon", "Timezone"]);
            for (alias, location) in places {
                table.add_row([
                    alias.clone(),
                    location.name.clone(),
                    location.country_code.clone().unwrap_or_default(),
                    location.latitude.to_string(),
                    location.longitude.to_string(),
                    location.timezone.clone().unwrap_or_default(),
                ]);
            }
        }
        TableData::Current(envelope) => {
            table.set_header(["Field", "Value"]);
            if let Some(current) = &envelope.current {
                for (key, value) in current {
                    table.add_row([key.clone(), value.to_string()]);
                }
            }
        }
        TableData::Daily(envelope) => add_series_table(&mut table, envelope.daily.as_ref())?,
        TableData::Hourly(envelope) => add_series_table(&mut table, envelope.hourly.as_ref())?,
        TableData::Signal(signal) => {
            add_signal_table(&mut table, signal)?;
        }
        TableData::BatchSignal(batch) => {
            table.set_header([
                "Input", "Location", "Country", "Date", "Max C", "Min C", "Rain %", "Rain mm",
                "Flags", "Error",
            ]);
            for item in &batch.items {
                if let Some(signal) = &item.signal {
                    for day in &signal.days {
                        table.add_row([
                            item.input.clone(),
                            signal.location.name.clone(),
                            signal.location.country_code.clone().unwrap_or_default(),
                            day.date.clone(),
                            fmt_opt(day.temp_max_c),
                            fmt_opt(day.temp_min_c),
                            day.precip_probability_max_pct
                                .map(|v| v.to_string())
                                .unwrap_or_default(),
                            fmt_opt(day.precipitation_mm),
                            flag_list(&day.flags),
                            String::new(),
                        ]);
                    }
                } else {
                    table.add_row([
                        item.input.clone(),
                        String::new(),
                        item.country.clone().unwrap_or_default(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        item.error.clone().unwrap_or_default(),
                    ]);
                }
            }
        }
        TableData::Threshold(threshold) => {
            table.set_header([
                "Date",
                "Reasons",
                "Max C",
                "Min C",
                "Rain %",
                "Rain mm",
                "Wind Gust",
                "Flags",
            ]);
            for item in &threshold.matches {
                table.add_row([
                    item.date.clone(),
                    item.reasons.join("|"),
                    fmt_opt(item.signal.temp_max_c),
                    fmt_opt(item.signal.temp_min_c),
                    item.signal
                        .precip_probability_max_pct
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    fmt_opt(item.signal.precipitation_mm),
                    fmt_opt(item.signal.wind_gust_max_kmh),
                    flag_list(&item.signal.flags),
                ]);
            }
        }
        TableData::Summary(summary) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["headline".to_string(), summary.headline.clone()]);
            table.add_row(["total_days".to_string(), summary.total_days.to_string()]);
            table.add_row(["risk_days".to_string(), summary.risk_days.to_string()]);
            table.add_row(["warm_days".to_string(), summary.warm_days.to_string()]);
            table.add_row(["hot_days".to_string(), summary.hot_days.to_string()]);
            table.add_row(["wet_days".to_string(), summary.wet_days.to_string()]);
            table.add_row(["windy_days".to_string(), summary.windy_days.to_string()]);
            table.add_row(["sunny_days".to_string(), summary.sunny_days.to_string()]);
        }
        TableData::CacheStatus(status) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["Path".to_string(), status.path.display().to_string()]);
            table.add_row(["Files".to_string(), status.files.to_string()]);
            table.add_row(["Bytes".to_string(), status.bytes.to_string()]);
        }
        TableData::Message(message) => {
            table.set_header(["Status"]);
            table.add_row([message]);
        }
    }
    Ok(table.to_string())
}

fn to_csv(data: TableData<'_>) -> Result<()> {
    let mut writer = csv::Writer::from_writer(std::io::stdout());
    match data {
        TableData::Geocode(locations) => {
            writer.write_record([
                "name",
                "country_code",
                "admin1",
                "latitude",
                "longitude",
                "timezone",
            ])?;
            for location in locations {
                writer.write_record([
                    &location.name,
                    location.country_code.as_deref().unwrap_or(""),
                    location.admin1.as_deref().unwrap_or(""),
                    &location.latitude.to_string(),
                    &location.longitude.to_string(),
                    location.timezone.as_deref().unwrap_or(""),
                ])?;
            }
        }
        TableData::Places(places) => {
            writer.write_record([
                "alias",
                "name",
                "country_code",
                "latitude",
                "longitude",
                "timezone",
            ])?;
            for (alias, location) in places {
                writer.write_record([
                    alias,
                    &location.name,
                    location.country_code.as_deref().unwrap_or(""),
                    &location.latitude.to_string(),
                    &location.longitude.to_string(),
                    location.timezone.as_deref().unwrap_or(""),
                ])?;
            }
        }
        TableData::Daily(envelope) => write_series_csv(&mut writer, envelope.daily.as_ref())?,
        TableData::Hourly(envelope) => write_series_csv(&mut writer, envelope.hourly.as_ref())?,
        TableData::Signal(signal) => {
            write_signal_csv(&mut writer, signal)?;
        }
        TableData::BatchSignal(batch) => {
            writer.write_record([
                "input",
                "location",
                "country_code",
                "date",
                "temp_max_c",
                "temp_min_c",
                "precip_probability_max_pct",
                "precipitation_mm",
                "wind_gust_max_kmh",
                "sunshine_hours",
                "flags",
                "error",
            ])?;
            for item in &batch.items {
                if let Some(signal) = &item.signal {
                    for day in &signal.days {
                        writer.write_record([
                            &item.input,
                            &signal.location.name,
                            signal.location.country_code.as_deref().unwrap_or(""),
                            &day.date,
                            &fmt_opt(day.temp_max_c),
                            &fmt_opt(day.temp_min_c),
                            &day.precip_probability_max_pct
                                .map(|v| v.to_string())
                                .unwrap_or_default(),
                            &fmt_opt(day.precipitation_mm),
                            &fmt_opt(day.wind_gust_max_kmh),
                            &fmt_opt(day.sunshine_hours),
                            &flag_list(&day.flags),
                            "",
                        ])?;
                    }
                } else {
                    writer.write_record([
                        &item.input,
                        "",
                        item.country.as_deref().unwrap_or(""),
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        item.error.as_deref().unwrap_or(""),
                    ])?;
                }
            }
        }
        TableData::Threshold(threshold) => {
            writer.write_record([
                "date",
                "reasons",
                "temp_max_c",
                "temp_min_c",
                "precip_probability_max_pct",
                "precipitation_mm",
                "wind_gust_max_kmh",
                "flags",
            ])?;
            for item in &threshold.matches {
                writer.write_record([
                    &item.date,
                    &item.reasons.join("|"),
                    &fmt_opt(item.signal.temp_max_c),
                    &fmt_opt(item.signal.temp_min_c),
                    &item
                        .signal
                        .precip_probability_max_pct
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    &fmt_opt(item.signal.precipitation_mm),
                    &fmt_opt(item.signal.wind_gust_max_kmh),
                    &flag_list(&item.signal.flags),
                ])?;
            }
        }
        TableData::Summary(summary) => {
            writer.write_record(["field", "value"])?;
            writer.write_record(["headline", &summary.headline])?;
            writer.write_record(["total_days", &summary.total_days.to_string()])?;
            writer.write_record(["risk_days", &summary.risk_days.to_string()])?;
            writer.write_record(["warm_days", &summary.warm_days.to_string()])?;
            writer.write_record(["hot_days", &summary.hot_days.to_string()])?;
            writer.write_record(["wet_days", &summary.wet_days.to_string()])?;
            writer.write_record(["windy_days", &summary.windy_days.to_string()])?;
            writer.write_record(["sunny_days", &summary.sunny_days.to_string()])?;
        }
        TableData::Current(envelope) => {
            writer.write_record(["field", "value"])?;
            if let Some(current) = &envelope.current {
                for (key, value) in current {
                    writer.write_record([key, &value.to_string()])?;
                }
            }
        }
        TableData::CacheStatus(status) => {
            writer.write_record(["path", "files", "bytes"])?;
            writer.write_record([
                &status.path.display().to_string(),
                &status.files.to_string(),
                &status.bytes.to_string(),
            ])?;
        }
        TableData::Message(message) => {
            writer.write_record(["status"])?;
            writer.write_record([message])?;
        }
    }
    writer.flush()?;
    Ok(())
}

fn add_series_table(table: &mut Table, series: Option<&Series>) -> Result<()> {
    let series = series.ok_or_else(|| anyhow!("series data missing"))?;
    let mut keys: Vec<_> = series.values.keys().cloned().collect();
    keys.sort();
    let mut header = vec!["time".to_string()];
    header.extend(keys.iter().cloned());
    table.set_header(header);
    for idx in 0..series.len() {
        let mut row = vec![series.time[idx].clone()];
        for key in &keys {
            row.push(
                series
                    .values
                    .get(key)
                    .and_then(|values| values.get(idx))
                    .map(value_string)
                    .unwrap_or_default(),
            );
        }
        table.add_row(row);
    }
    Ok(())
}

fn write_series_csv(
    writer: &mut csv::Writer<std::io::Stdout>,
    series: Option<&Series>,
) -> Result<()> {
    let series = series.ok_or_else(|| anyhow!("series data missing"))?;
    let mut keys: Vec<_> = series.values.keys().cloned().collect();
    keys.sort();
    let mut header = vec!["time".to_string()];
    header.extend(keys.iter().cloned());
    writer.write_record(header)?;
    for idx in 0..series.len() {
        let mut row = vec![series.time[idx].clone()];
        for key in &keys {
            row.push(
                series
                    .values
                    .get(key)
                    .and_then(|values| values.get(idx))
                    .map(value_string)
                    .unwrap_or_default(),
            );
        }
        writer.write_record(row)?;
    }
    Ok(())
}

fn add_signal_table(table: &mut Table, signal: &SignalEnvelope) -> Result<()> {
    table.set_header([
        "Date",
        "Max C",
        "Min C",
        "Rain %",
        "Rain mm",
        "Wind Gust",
        "Sun h",
        "Flags",
    ]);
    for day in &signal.days {
        table.add_row([
            day.date.clone(),
            fmt_opt(day.temp_max_c),
            fmt_opt(day.temp_min_c),
            day.precip_probability_max_pct
                .map(|v| v.to_string())
                .unwrap_or_default(),
            fmt_opt(day.precipitation_mm),
            fmt_opt(day.wind_gust_max_kmh),
            fmt_opt(day.sunshine_hours),
            flag_list(&day.flags),
        ]);
    }
    Ok(())
}

fn write_signal_csv(
    writer: &mut csv::Writer<std::io::Stdout>,
    signal: &SignalEnvelope,
) -> Result<()> {
    writer.write_record([
        "date",
        "temp_max_c",
        "temp_min_c",
        "precip_probability_max_pct",
        "precipitation_mm",
        "wind_gust_max_kmh",
        "sunshine_hours",
        "flags",
    ])?;
    for day in &signal.days {
        writer.write_record([
            &day.date,
            &fmt_opt(day.temp_max_c),
            &fmt_opt(day.temp_min_c),
            &day.precip_probability_max_pct
                .map(|v| v.to_string())
                .unwrap_or_default(),
            &fmt_opt(day.precipitation_mm),
            &fmt_opt(day.wind_gust_max_kmh),
            &fmt_opt(day.sunshine_hours),
            &flag_list(&day.flags),
        ])?;
    }
    Ok(())
}

fn value_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn fmt_opt(value: Option<f64>) -> String {
    value.map(|v| format!("{v:.2}")).unwrap_or_default()
}

fn flag_list(flags: &DemandFlags) -> String {
    [
        ("rain_likely", flags.rain_likely),
        ("wet_day", flags.wet_day),
        ("heavy_rain", flags.heavy_rain),
        ("warm_day", flags.warm_day),
        ("hot_day", flags.hot_day),
        ("cold_day", flags.cold_day),
        ("windy_day", flags.windy_day),
        ("sunny_day", flags.sunny_day),
        ("high_uv", flags.high_uv),
    ]
    .into_iter()
    .filter_map(|(name, active)| active.then_some(name))
    .collect::<Vec<_>>()
    .join("|")
}

fn parse_lat_lon(input: &str) -> Option<(f64, f64)> {
    let (lat, lon) = input.split_once(',')?;
    let lat = lat.trim().parse().ok()?;
    let lon = lon.trim().parse().ok()?;
    Some((lat, lon))
}

fn validate_coordinates(lat: f64, lon: f64) -> Result<()> {
    if !lat.is_finite() || !lon.is_finite() {
        return Err(anyhow!("latitude and longitude must be finite numbers"));
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(anyhow!("latitude must be between -90 and 90"));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(anyhow!("longitude must be between -180 and 180"));
    }
    Ok(())
}

fn parse_date(input: &str, name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(input, "%Y-%m-%d")
        .with_context(|| format!("{name} date must use YYYY-MM-DD"))
}

fn default_config_path() -> PathBuf {
    xdg_home("XDG_CONFIG_HOME", ".config")
        .join(APP_NAME)
        .join("config.toml")
}

fn default_cache_dir() -> PathBuf {
    xdg_home("XDG_CACHE_HOME", ".cache").join(APP_NAME)
}

fn xdg_home(var: &str, fallback: &str) -> PathBuf {
    env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(fallback))
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lat_lon() {
        assert_eq!(parse_lat_lon("51.5,-0.12"), Some((51.5, -0.12)));
        assert_eq!(parse_lat_lon(" 51.5 , -0.12 "), Some((51.5, -0.12)));
        assert_eq!(parse_lat_lon("51.5"), None);
    }

    #[test]
    fn validates_coordinate_bounds() {
        assert!(validate_coordinates(51.5, -0.12).is_ok());
        assert!(validate_coordinates(91.0, -0.12).is_err());
        assert!(validate_coordinates(51.5, 181.0).is_err());
        assert!(validate_coordinates(f64::NAN, -0.12).is_err());
    }

    #[test]
    fn identifies_loopback_http_hosts() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("::1"));
        assert!(!is_loopback_host("0.0.0.0"));
    }

    #[test]
    fn normalizes_http_paths() {
        assert_eq!(normalized_http_path("mcp"), "/mcp");
        assert_eq!(normalized_http_path("/mcp"), "/mcp");
        assert_eq!(normalized_http_path(""), "/");
        assert_eq!(normalized_http_path("   "), "/");
    }

    #[test]
    fn parses_batch_concurrency_bounds() {
        assert_eq!(parse_batch_concurrency("1").unwrap(), 1);
        assert_eq!(parse_batch_concurrency("32").unwrap(), 32);
        assert!(parse_batch_concurrency("0").is_err());
        assert!(parse_batch_concurrency("33").is_err());
        assert!(parse_batch_concurrency("many").is_err());
    }

    #[test]
    fn signal_thresholds_match_plan() {
        let envelope = sample_forecast_envelope();
        let signal = SignalEnvelope::from_forecast(envelope, SignalProfile::Demand).unwrap();
        let flags = &signal.days[0].flags;
        assert!(flags.rain_likely);
        assert!(flags.wet_day);
        assert!(flags.heavy_rain);
        assert!(flags.warm_day);
        assert!(flags.hot_day);
        assert!(flags.cold_day);
        assert!(flags.windy_day);
        assert!(flags.sunny_day);
        assert!(flags.high_uv);
    }

    #[test]
    fn threshold_criteria_reports_matching_reasons() {
        let signal =
            SignalEnvelope::from_forecast(sample_forecast_envelope(), SignalProfile::Demand)
                .unwrap();
        let criteria = ThresholdCriteria {
            rain_prob_gte: Some(50),
            precip_mm_gte: Some(10.0),
            temp_max_gte: Some(25.0),
            temp_min_lte: None,
            wind_gust_gte: Some(40.0),
        };
        assert_eq!(
            criteria.match_reasons(&signal.days[0]),
            vec!["rain_prob_gte", "temp_max_gte", "wind_gust_gte"]
        );
    }

    #[test]
    fn threshold_criteria_requires_a_condition() {
        let args = ThresholdArgs {
            location: "London".to_string(),
            country: None,
            days: 7,
            rain_prob_gte: None,
            precip_mm_gte: None,
            temp_max_gte: None,
            temp_min_lte: None,
            wind_gust_gte: None,
        };
        let error = ThresholdCriteria::from_args(&args).unwrap_err();
        assert!(error.to_string().contains("at least one threshold"));
    }

    #[test]
    fn cache_round_trips_json_values() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf()).unwrap();
        let url = "https://api.open-meteo.com/v1/forecast?latitude=51.5&longitude=-0.1";
        let value = json!({"daily": {"time": ["2026-04-27"]}});

        assert!(cache.get(url, Duration::from_secs(60)).unwrap().is_none());
        cache.put(url, &value).unwrap();
        assert_eq!(
            cache.get(url, Duration::from_secs(60)).unwrap(),
            Some(value)
        );
        assert_eq!(cache.status().unwrap().files, 1);
    }

    #[test]
    fn cache_prune_keeps_recent_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf()).unwrap();
        cache
            .put("https://example.com/weather", &json!({"ok": true}))
            .unwrap();

        assert_eq!(cache.prune_older_than(Duration::from_secs(60)).unwrap(), 0);
        assert_eq!(cache.status().unwrap().files, 1);
    }

    #[test]
    fn parses_batch_locations_with_default_country() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("places.csv");
        fs::write(&path, "location,country\nLondon,\nParis,FR\n").unwrap();
        let app = test_app(dir.path());

        let locations = app.batch_locations_from_csv(&path, Some("GB")).unwrap();

        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].location, "London");
        assert_eq!(locations[0].country.as_deref(), Some("GB"));
        assert_eq!(locations[1].location, "Paris");
        assert_eq!(locations[1].country.as_deref(), Some("FR"));
    }

    #[test]
    fn summary_counts_signal_flags() {
        let signal =
            SignalEnvelope::from_forecast(sample_forecast_envelope(), SignalProfile::Demand)
                .unwrap();
        let summary = SummaryEnvelope::from_signal(signal);
        assert_eq!(summary.total_days, 1);
        assert_eq!(summary.risk_days, 1);
        assert_eq!(summary.warm_days, 1);
        assert_eq!(summary.hot_days, 1);
        assert_eq!(summary.wet_days, 1);
        assert_eq!(summary.windy_days, 1);
        assert_eq!(summary.sunny_days, 1);
    }

    #[test]
    fn config_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = Config {
            places: BTreeMap::from([(
                "london".to_string(),
                Location {
                    id: Some(1),
                    name: "London".to_string(),
                    country: Some("United Kingdom".to_string()),
                    country_code: Some("GB".to_string()),
                    admin1: Some("England".to_string()),
                    latitude: 51.50853,
                    longitude: -0.12574,
                    timezone: Some("Europe/London".to_string()),
                    population: Some(8961989),
                },
            )]),
        };
        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.places["london"].country_code.as_deref(), Some("GB"));
    }

    fn sample_forecast_envelope() -> ForecastEnvelope {
        ForecastEnvelope {
            source: "open-meteo".to_string(),
            location: Location {
                id: None,
                name: "London".to_string(),
                country: None,
                country_code: Some("GB".to_string()),
                admin1: None,
                latitude: 51.5,
                longitude: -0.1,
                timezone: Some("Europe/London".to_string()),
                population: None,
            },
            fetched_at: Utc::now(),
            cache: CacheState::Miss,
            timezone: "Europe/London".to_string(),
            current: None,
            hourly: None,
            daily: Some(Series {
                time: vec!["2026-04-27".to_string()],
                values: BTreeMap::from([
                    ("temperature_2m_max".to_string(), vec![json!(25.0)]),
                    ("temperature_2m_min".to_string(), vec![json!(5.0)]),
                    ("precipitation_sum".to_string(), vec![json!(5.0)]),
                    ("precipitation_probability_max".to_string(), vec![json!(50)]),
                    ("wind_gusts_10m_max".to_string(), vec![json!(40.0)]),
                    ("sunshine_duration".to_string(), vec![json!(21600.0)]),
                    ("uv_index_max".to_string(), vec![json!(6.0)]),
                    ("weather_code".to_string(), vec![json!(61)]),
                ]),
            }),
        }
    }

    fn test_app(root: &Path) -> App {
        App {
            client: reqwest::Client::new(),
            config: Config::default(),
            config_path: root.join("config.toml"),
            cache: Cache::new(root.join("cache")).unwrap(),
            forecast_base_url: "https://api.open-meteo.com/v1/forecast".to_string(),
            geocode_base_url: "https://geocoding-api.open-meteo.com/v1/search".to_string(),
            historical_base_url: "https://archive-api.open-meteo.com/v1/archive".to_string(),
            api_key: None,
            refresh: false,
            cache_ttl: Duration::from_secs(1800),
        }
    }
}
