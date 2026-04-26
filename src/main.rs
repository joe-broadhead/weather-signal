use anyhow::{Result, anyhow};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use serde_json::json;
use std::process;
use tracing_subscriber::{EnvFilter, fmt};

mod app;
mod cache;
mod cli;
mod mcp;
mod models;
mod output;
mod signals;
mod util;

use app::App;
use cli::{
    BatchCommand, CacheCommand, Cli, Command, OutputFormat, PlacesCommand, ServerArgs,
    ServerCommand,
};
use mcp::start_mcp_server;
use models::ForecastKind;
use output::{TableData, render};
use util::parse_date;

pub(crate) const APP_NAME: &str = "weather-signal";
pub(crate) const DEFAULT_FORECAST_BASE_URL: &str = "https://api.open-meteo.com/v1/forecast";
pub(crate) const DEFAULT_GEOCODE_BASE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
pub(crate) const DEFAULT_HISTORICAL_BASE_URL: &str =
    "https://archive-api.open-meteo.com/v1/archive";
pub(crate) const DEFAULT_BATCH_CONCURRENCY: usize = 4;

#[tokio::main]
async fn main() {
    init_tracing();
    let cli = Cli::parse();
    let output = cli.output_format();
    if let Err(err) = run(cli).await {
        if output == OutputFormat::Json {
            let causes = err
                .chain()
                .skip(1)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            eprintln!("{}", json!({"error": err.to_string(), "causes": causes}));
        } else {
            eprintln!("error: {err:#}");
        }
        process::exit(1);
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
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
