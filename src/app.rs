use crate::{
    cache::{Cache, CacheState},
    cli::{BatchPlaces, BatchSignalArgs, Cli, SignalProfile, SummaryArgs, ThresholdArgs},
    models::{
        BatchLocationInput, BatchLocationRecord, BatchSignalEnvelope, BatchSignalItem, Cached,
        Config, ForecastEnvelope, ForecastKind, ForecastResponse, GeocodeResponse, Location,
    },
    signals::{
        SignalEnvelope, SummaryEnvelope, ThresholdCriteria, ThresholdEnvelope, ThresholdMatch,
    },
    util::{
        default_cache_dir, default_config_path, parse_lat_lon, redact_url, validate_coordinates,
    },
};
use anyhow::{Context, Result, anyhow};
use chrono::{NaiveDate, Utc};
use futures::{StreamExt, stream};
use reqwest::{StatusCode, header};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::time::sleep;
use tracing::{debug, warn};
use url::Url;

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
const MAX_REQUEST_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub(crate) struct App {
    pub(crate) client: reqwest::Client,
    pub(crate) config: Config,
    pub(crate) config_path: PathBuf,
    pub(crate) cache: Cache,
    pub(crate) forecast_base_url: String,
    pub(crate) geocode_base_url: String,
    pub(crate) historical_base_url: String,
    pub(crate) api_key: Option<String>,
    pub(crate) refresh: bool,
    pub(crate) cache_ttl: Duration,
}

impl App {
    pub(crate) async fn new(cli: &Cli) -> Result<Self> {
        let config_path = cli.config.clone().unwrap_or_else(default_config_path);
        let config = Config::load(&config_path)?;
        validate_base_url(&cli.forecast_base_url, "forecast")?;
        validate_base_url(&cli.geocode_base_url, "geocoding")?;
        validate_base_url(&cli.historical_base_url, "historical")?;
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(cli.timeout)
                .connect_timeout(Duration::from_secs(10))
                .user_agent(format!("weather-signal/{}", env!("CARGO_PKG_VERSION")))
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

    pub(crate) async fn geocode(
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

    pub(crate) async fn resolve_geocoded(
        &self,
        query: &str,
        country: Option<&str>,
    ) -> Result<Location> {
        let location = self
            .geocode(query, country, 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no geocoding result for {query:?}"))?;
        validate_coordinates(location.latitude, location.longitude)
            .with_context(|| format!("geocoding returned invalid coordinates for {query:?}"))?;
        Ok(location)
    }

    pub(crate) async fn resolve_location(
        &self,
        input: &str,
        country: Option<&str>,
    ) -> Result<Location> {
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

    pub(crate) async fn forecast(
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

    pub(crate) async fn historical(
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

    pub(crate) async fn signal_for(
        &self,
        location: &str,
        country: Option<&str>,
        days: u8,
        profile: SignalProfile,
    ) -> Result<SignalEnvelope> {
        let resolved = self.resolve_location(location, country).await?;
        let envelope = self
            .forecast(&resolved, ForecastKind::Daily, days, None)
            .await?;
        SignalEnvelope::from_forecast(envelope, profile)
    }

    pub(crate) async fn batch_signal(&self, args: &BatchSignalArgs) -> Result<BatchSignalEnvelope> {
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
                .forecast(&location, ForecastKind::Daily, days, None)
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

    pub(crate) async fn threshold(&self, args: &ThresholdArgs) -> Result<ThresholdEnvelope> {
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

    pub(crate) async fn summary(&self, args: &SummaryArgs) -> Result<SummaryEnvelope> {
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
            match serde_json::from_value(value) {
                Ok(value) => {
                    debug!(url = %redact_url(url), "cache hit");
                    return Ok(Cached {
                        value,
                        state: CacheState::Hit,
                    });
                }
                Err(error) => {
                    let removed = self.cache.remove(url)?;
                    warn!(
                        url = %redact_url(url),
                        removed,
                        error = %error,
                        "cached response failed to decode; evicting and refetching"
                    );
                }
            }
        }
        debug!(url = %redact_url(url), refresh = self.refresh, "cache miss");
        let value = self.request_json_with_retries(url).await?;
        self.cache.put(url, &value)?;
        let state = if self.refresh {
            CacheState::Refresh
        } else {
            CacheState::Miss
        };
        let value = serde_json::from_value(value).context("failed to decode API response")?;
        Ok(Cached { value, state })
    }

    async fn request_json_with_retries(&self, url: &str) -> Result<Value> {
        let mut last_error: Option<anyhow::Error> = None;
        for attempt in 1..=MAX_REQUEST_ATTEMPTS {
            match self.request_json_once(url).await {
                Ok(value) => return Ok(value),
                Err(error) if attempt < MAX_REQUEST_ATTEMPTS && is_retryable_error(&error) => {
                    warn!(url = %redact_url(url), attempt, error = %error, "transient weather request failed; retrying");
                    last_error = Some(error);
                    sleep(
                        last_error
                            .as_ref()
                            .and_then(retry_after)
                            .unwrap_or_else(|| retry_delay(attempt)),
                    )
                    .await;
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("weather request failed after {attempt} attempt(s)")
                    });
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow!("weather request failed"))).with_context(|| {
            format!("weather request failed after {MAX_REQUEST_ATTEMPTS} attempts")
        })
    }

    async fn request_json_once(&self, url: &str) -> Result<Value> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("request failed")?;
        let status = response.status();
        if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_retry_after);
            return Err(TransientStatus {
                status,
                retry_after,
            }
            .into());
        }
        response
            .error_for_status()
            .context("API returned an error status")?
            .json()
            .await
            .context("failed to decode API response")
    }

    fn append_api_key(&self, url: &mut Url) {
        if let Some(key) = self.api_key.as_deref().filter(|key| !key.is_empty()) {
            url.query_pairs_mut().append_pair("apikey", key);
        }
    }
}

fn is_retryable_error(error: &anyhow::Error) -> bool {
    if let Some(error) = error.downcast_ref::<reqwest::Error>() {
        return error.is_timeout() || error.is_connect() || error.is_request();
    }
    error.downcast_ref::<TransientStatus>().is_some()
}

fn retry_after(error: &anyhow::Error) -> Option<Duration> {
    error
        .downcast_ref::<TransientStatus>()
        .and_then(|error| error.retry_after)
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(150 * 2_u64.pow((attempt - 1) as u32))
}

fn parse_retry_after(input: &str) -> Option<Duration> {
    let seconds = input.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds.min(30)))
}

fn validate_base_url(input: &str, name: &str) -> Result<()> {
    let url = Url::parse(input).with_context(|| format!("{name} base URL is invalid"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(anyhow!("{name} base URL must use http or https"));
    }
    Ok(())
}

#[derive(Debug)]
struct TransientStatus {
    status: StatusCode,
    retry_after: Option<Duration>,
}

impl fmt::Display for TransientStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "API returned transient status {}", self.status)
    }
}

impl Error for TransientStatus {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEFAULT_FORECAST_BASE_URL, DEFAULT_GEOCODE_BASE_URL, DEFAULT_HISTORICAL_BASE_URL};
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[test]
    fn parses_batch_locations_with_default_country() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("places.csv");
        std::fs::write(&path, "location,country\nLondon,\nParis,FR\n").unwrap();
        let app = test_app(dir.path());

        let locations = app.batch_locations_from_csv(&path, Some("GB")).unwrap();

        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].location, "London");
        assert_eq!(locations[0].country.as_deref(), Some("GB"));
        assert_eq!(locations[1].location, "Paris");
        assert_eq!(locations[1].country.as_deref(), Some("FR"));
    }

    #[test]
    fn validates_base_url_scheme() {
        assert!(validate_base_url("https://example.com/v1/forecast", "forecast").is_ok());
        assert!(validate_base_url("http://127.0.0.1:8080/v1/forecast", "forecast").is_ok());
        assert!(validate_base_url("file:///tmp/forecast", "forecast").is_err());
    }

    #[test]
    fn typed_transient_status_is_retryable() {
        let error = anyhow!(TransientStatus {
            status: StatusCode::TOO_MANY_REQUESTS,
            retry_after: Some(Duration::from_secs(1)),
        });
        assert!(is_retryable_error(&error));
        assert_eq!(retry_after(&error), Some(Duration::from_secs(1)));
    }

    #[tokio::test]
    async fn retries_429_then_succeeds() {
        let base_url = spawn_sequence_server(vec![
            "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\n\r\n"
                .to_string(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                "{}".len(),
                "{}"
            ),
        ])
        .await;
        let app = test_app_with_base(tempfile::tempdir().unwrap().path(), &base_url);

        assert_eq!(
            app.request_json_with_retries(&base_url).await.unwrap(),
            json!({})
        );
    }

    #[tokio::test]
    async fn poisoned_cache_entry_is_evicted_and_refetched() {
        let body = json!({
            "timezone": "Europe/London",
            "daily": {"time": ["2026-04-27"], "temperature_2m_max": [20.0]}
        })
        .to_string();
        let base_url = spawn_sequence_server(vec![format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )])
        .await;
        let dir = tempfile::tempdir().unwrap();
        let app = test_app_with_base(dir.path(), &base_url);
        let location = Location {
            id: None,
            name: "London".to_string(),
            country: None,
            country_code: Some("GB".to_string()),
            admin1: None,
            latitude: 51.5,
            longitude: -0.1,
            timezone: None,
            population: None,
        };
        let url = format!(
            "{base_url}?latitude=51.5&longitude=-0.1&forecast_days=1&timezone=auto&daily={}",
            DAILY_VARS.join("%2C")
        );
        app.cache.put(&url, &json!({"bad": "shape"})).unwrap();

        let envelope = app
            .forecast(&location, ForecastKind::Daily, 1, None)
            .await
            .unwrap();

        assert_eq!(envelope.cache, CacheState::Miss);
        assert!(envelope.daily.is_some());
    }

    fn test_app(root: &Path) -> App {
        test_app_with_base(root, DEFAULT_FORECAST_BASE_URL)
    }

    fn test_app_with_base(root: &Path, forecast_base_url: &str) -> App {
        App {
            client: reqwest::Client::new(),
            config: Config::default(),
            config_path: root.join("config.toml"),
            cache: Cache::new(root.join("cache")).unwrap(),
            forecast_base_url: forecast_base_url.to_string(),
            geocode_base_url: DEFAULT_GEOCODE_BASE_URL.to_string(),
            historical_base_url: DEFAULT_HISTORICAL_BASE_URL.to_string(),
            api_key: None,
            refresh: false,
            cache_ttl: Duration::from_secs(1800),
        }
    }

    async fn spawn_sequence_server(responses: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for response in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buffer = [0_u8; 2048];
                let _ = stream.read(&mut buffer).await.unwrap();
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        format!("http://{address}/v1/forecast")
    }
}
