use crate::{
    APP_NAME,
    app::App,
    cli::{ServerStartArgs, ServerTransport, SignalProfile, SummaryArgs, ThresholdArgs},
    models::ForecastKind,
    util::parse_date,
};
use anyhow::{Context, Result, anyhow};
use axum::{Json, Router, http::StatusCode, routing::get};
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
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io, sync::Arc, time::Duration};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

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
            "error": format!("{error:#}")
        })
        .to_string()
    }

    async fn handle<T, Fut>(&self, future: Fut) -> std::result::Result<String, String>
    where
        T: Serialize,
        Fut: std::future::Future<Output = Result<T>>,
    {
        match future.await {
            Ok(value) => Ok(Self::success(&value)),
            Err(error) => Err(Self::error(error)),
        }
    }
}

#[tool_router]
impl WeatherMcpServer {
    #[tool(
        name = "geocode",
        description = "Resolve a place name using Open-Meteo geocoding. Use before weather tools when location names are ambiguous."
    )]
    async fn geocode_tool(
        &self,
        params: Parameters<GeocodeToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn current_weather(
        &self,
        params: Parameters<LocationToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn daily_forecast(
        &self,
        params: Parameters<DaysToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn hourly_forecast(
        &self,
        params: Parameters<HourlyToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn demand_signal(
        &self,
        params: Parameters<DaysToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn weather_summary(
        &self,
        params: Parameters<DaysToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn threshold_days(
        &self,
        params: Parameters<ThresholdToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn historical_weather(
        &self,
        params: Parameters<HistoricalToolParams>,
    ) -> std::result::Result<String, String> {
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
    async fn list_places(&self) -> std::result::Result<String, String> {
        Ok(Self::success(&self.app.config.places))
    }

    #[tool(
        name = "cache_status",
        description = "Inspect the local weather response cache path, file count, and byte size."
    )]
    async fn cache_status(&self) -> std::result::Result<String, String> {
        match self.app.cache.status() {
            Ok(status) => Ok(Self::success(&status)),
            Err(error) => Err(Self::error(error)),
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

pub(crate) async fn start_mcp_server(app: App, args: &ServerStartArgs) -> Result<()> {
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
    if matches!(path.as_str(), "/healthz" | "/readyz") {
        return Err(anyhow!(
            "--http-path {path} conflicts with built-in health endpoints"
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
