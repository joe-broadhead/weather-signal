use crate::{
    DEFAULT_BATCH_CONCURRENCY, DEFAULT_FORECAST_BASE_URL, DEFAULT_GEOCODE_BASE_URL,
    DEFAULT_HISTORICAL_BASE_URL,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::{path::PathBuf, time::Duration};

#[derive(Parser, Debug)]
#[command(
    name = crate::APP_NAME,
    version,
    about = "Agent-first Open-Meteo weather signal CLI"
)]
pub(crate) struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json, help = "Output format")]
    pub(crate) output: OutputFormat,
    #[arg(
        long,
        global = true,
        conflicts_with = "output",
        help = "Shortcut for --output table"
    )]
    pub(crate) table: bool,
    #[arg(
        long,
        global = true,
        help = "Bypass cache reads and write fresh API responses"
    )]
    pub(crate) refresh: bool,
    #[arg(long, global = true, default_value = "30m", value_parser = parse_duration, help = "Forecast cache TTL, such as 10m or 1h; geocoding and historical archive use fixed TTLs")]
    pub(crate) cache_ttl: Duration,
    #[arg(long, global = true, default_value = "30s", value_parser = parse_duration, help = "HTTP request timeout, such as 15s")]
    pub(crate) timeout: Duration,
    #[arg(
        long,
        global = true,
        env = "OPEN_METEO_API_KEY",
        hide_env_values = true,
        help = "Open-Meteo API key for commercial endpoints"
    )]
    pub(crate) api_key: Option<String>,
    #[arg(long, global = true, env = "OPEN_METEO_FORECAST_BASE_URL", default_value = DEFAULT_FORECAST_BASE_URL, help = "Open-Meteo forecast endpoint override")]
    pub(crate) forecast_base_url: String,
    #[arg(long, global = true, env = "OPEN_METEO_GEOCODING_BASE_URL", default_value = DEFAULT_GEOCODE_BASE_URL, help = "Open-Meteo geocoding endpoint override")]
    pub(crate) geocode_base_url: String,
    #[arg(long, global = true, env = "OPEN_METEO_HISTORICAL_BASE_URL", default_value = DEFAULT_HISTORICAL_BASE_URL, help = "Open-Meteo historical archive endpoint override")]
    pub(crate) historical_base_url: String,
    #[arg(long, global = true, help = "Path to the saved places TOML config")]
    pub(crate) config: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

impl Cli {
    pub(crate) fn output_format(&self) -> OutputFormat {
        if self.table {
            OutputFormat::Table
        } else {
            self.output
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Table,
    Csv,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
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
pub(crate) enum PlacesCommand {
    #[command(about = "Resolve and save a location alias")]
    Add(PlaceAddArgs),
    #[command(about = "List saved location aliases")]
    List,
    #[command(about = "Remove a saved location alias")]
    Remove(PlaceRemoveArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum CacheCommand {
    #[command(about = "Show local cache path, file count, and bytes")]
    Status,
    #[command(about = "Remove local cache files")]
    Clear,
    #[command(about = "Remove local cache files older than a duration")]
    Prune(CachePruneArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum BatchCommand {
    #[command(about = "Run demand signals for saved places or CSV locations")]
    Signal(BatchSignalArgs),
}

#[derive(Args, Debug)]
pub(crate) struct GeocodeArgs {
    #[arg(help = "Place name to search")]
    pub(crate) query: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u16).range(1..=100), help = "Maximum geocoding candidates to return")]
    pub(crate) count: u16,
}

#[derive(Args, Debug)]
pub(crate) struct CachePruneArgs {
    #[arg(
        long,
        value_parser = parse_duration,
        default_value = "7d",
        help = "Remove cache files older than this duration, such as 24h or 7d"
    )]
    pub(crate) max_age: Duration,
}

#[derive(Args, Debug)]
pub(crate) struct PlaceAddArgs {
    #[arg(help = "Alias to save in the local config")]
    pub(crate) alias: String,
    #[arg(help = "Place name or search query to resolve")]
    pub(crate) query: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
}

#[derive(Args, Debug)]
pub(crate) struct PlaceRemoveArgs {
    #[arg(help = "Saved alias to remove")]
    pub(crate) alias: String,
}

#[derive(Args, Debug)]
pub(crate) struct LocationArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    pub(crate) location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
}

#[derive(Args, Debug)]
pub(crate) struct DailyArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    pub(crate) location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    pub(crate) days: u8,
}

#[derive(Args, Debug)]
pub(crate) struct HourlyArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    pub(crate) location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
    #[arg(long, default_value_t = 48, value_parser = clap::value_parser!(u16).range(1..=384), help = "Hourly horizon to return")]
    pub(crate) hours: u16,
}

#[derive(Args, Debug)]
pub(crate) struct SignalArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    pub(crate) location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    pub(crate) days: u8,
    #[arg(long, value_enum, default_value_t = SignalProfile::Demand, help = "Signal derivation profile")]
    pub(crate) profile: SignalProfile,
}

#[derive(Args, Debug)]
pub(crate) struct BatchSignalArgs {
    #[arg(long, value_enum, help = "Saved places source to use")]
    pub(crate) places: Option<BatchPlaces>,
    #[arg(long, help = "CSV with a location column and optional country column")]
    pub(crate) input: Option<PathBuf>,
    #[arg(long, help = "Default ISO 3166 country code for CSV rows")]
    pub(crate) country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    pub(crate) days: u8,
    #[arg(long, value_enum, default_value_t = SignalProfile::Demand, help = "Signal derivation profile")]
    pub(crate) profile: SignalProfile,
    #[arg(long, default_value_t = DEFAULT_BATCH_CONCURRENCY, value_parser = parse_batch_concurrency, help = "Maximum locations to fetch concurrently")]
    pub(crate) concurrency: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum BatchPlaces {
    All,
}

#[derive(Args, Debug)]
pub(crate) struct ThresholdArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    pub(crate) location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    pub(crate) days: u8,
    #[arg(
        long,
        help = "Match days with rain probability at or above this percent"
    )]
    pub(crate) rain_prob_gte: Option<i64>,
    #[arg(
        long,
        help = "Match days with precipitation at or above this millimeter total"
    )]
    pub(crate) precip_mm_gte: Option<f64>,
    #[arg(
        long,
        help = "Match days with max temperature at or above this Celsius value"
    )]
    pub(crate) temp_max_gte: Option<f64>,
    #[arg(
        long,
        help = "Match days with min temperature at or below this Celsius value"
    )]
    pub(crate) temp_min_lte: Option<f64>,
    #[arg(long, help = "Match days with wind gust at or above this km/h value")]
    pub(crate) wind_gust_gte: Option<f64>,
}

#[derive(Args, Debug)]
pub(crate) struct SummaryArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    pub(crate) location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=16), help = "Forecast horizon in days")]
    pub(crate) days: u8,
    #[arg(long, value_enum, default_value_t = SignalProfile::Demand, help = "Signal derivation profile")]
    pub(crate) profile: SignalProfile,
}

#[derive(Args, Debug)]
pub(crate) struct HistoricalArgs {
    #[arg(help = "Saved alias, place name, or lat,lon")]
    pub(crate) location: String,
    #[arg(long, help = "ISO 3166 country code hint, such as GB")]
    pub(crate) country: Option<String>,
    #[arg(long, help = "Start date in YYYY-MM-DD format")]
    pub(crate) start: String,
    #[arg(long, help = "End date in YYYY-MM-DD format")]
    pub(crate) end: String,
}

#[derive(Args, Debug)]
pub(crate) struct CompletionsArgs {
    #[arg(help = "Shell to generate completions for")]
    pub(crate) shell: Shell,
}

#[derive(Args, Debug)]
pub(crate) struct ServerArgs {
    #[command(subcommand)]
    pub(crate) command: ServerCommand,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ServerCommand {
    #[command(about = "Start MCP over stdio or streamable HTTP")]
    Start(ServerStartArgs),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ServerTransport {
    Stdio,
    StreamableHttp,
}

#[derive(Args, Debug)]
pub(crate) struct ServerStartArgs {
    #[arg(long, value_enum, default_value_t = ServerTransport::Stdio, help = "MCP transport to serve")]
    pub(crate) transport: ServerTransport,
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "HTTP bind host for streamable HTTP"
    )]
    pub(crate) http_host: String,
    #[arg(
        long,
        default_value_t = 8768,
        help = "HTTP bind port for streamable HTTP"
    )]
    pub(crate) http_port: u16,
    #[arg(long, default_value = "/mcp", help = "HTTP path for the MCP endpoint")]
    pub(crate) http_path: String,
    #[arg(
        long,
        default_value_t = false,
        help = "Enable stateful streamable HTTP sessions"
    )]
    pub(crate) http_stateful_mode: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum SignalProfile {
    Demand,
}

impl std::fmt::Display for SignalProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Demand => write!(f, "demand"),
        }
    }
}

pub(crate) fn parse_duration(
    input: &str,
) -> std::result::Result<Duration, humantime::DurationError> {
    humantime::parse_duration(input)
}

pub(crate) fn parse_batch_concurrency(input: &str) -> Result<usize, String> {
    let value = input
        .parse::<usize>()
        .map_err(|_| "concurrency must be an integer between 1 and 32".to_string())?;
    if !(1..=32).contains(&value) {
        return Err("concurrency must be between 1 and 32".to_string());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn parses_batch_concurrency_bounds() {
        assert_eq!(parse_batch_concurrency("1").unwrap(), 1);
        assert_eq!(parse_batch_concurrency("32").unwrap(), 32);
        assert!(parse_batch_concurrency("0").is_err());
        assert!(parse_batch_concurrency("33").is_err());
        assert!(parse_batch_concurrency("many").is_err());
    }

    #[test]
    fn api_key_env_value_is_hidden_from_help() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();

        assert!(help.contains("OPEN_METEO_API_KEY"));
        assert!(!help.contains("OPEN_METEO_API_KEY="));
    }
}
