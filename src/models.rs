use crate::cache::CacheState;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub(crate) enum ForecastKind {
    Current,
    Daily,
    Hourly,
}

pub(crate) struct Cached<T> {
    pub(crate) value: T,
    pub(crate) state: CacheState,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) places: BTreeMap<String, Location>,
}

impl Config {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("failed to parse config {}", path.display()))
    }

    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text).with_context(|| format!("failed to write config {}", path.display()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Location {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) country: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) admin1: Option<String>,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) timezone: Option<String>,
    pub(crate) population: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeocodeResponse {
    pub(crate) results: Option<Vec<Location>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ForecastResponse {
    pub(crate) timezone: String,
    pub(crate) current: Option<BTreeMap<String, Value>>,
    pub(crate) hourly: Option<Series>,
    pub(crate) daily: Option<Series>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ForecastEnvelope {
    pub(crate) source: String,
    pub(crate) location: Location,
    pub(crate) fetched_at: DateTime<Utc>,
    pub(crate) cache: CacheState,
    pub(crate) timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hourly: Option<Series>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) daily: Option<Series>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Series {
    pub(crate) time: Vec<String>,
    #[serde(flatten)]
    pub(crate) values: BTreeMap<String, Vec<Value>>,
}

impl Series {
    pub(crate) fn truncate(&mut self, limit: usize) {
        self.time.truncate(limit);
        for values in self.values.values_mut() {
            values.truncate(limit);
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.time.len()
    }

    pub(crate) fn get_f64(&self, key: &str, idx: usize) -> Option<f64> {
        self.values.get(key)?.get(idx)?.as_f64()
    }

    pub(crate) fn get_i64(&self, key: &str, idx: usize) -> Option<i64> {
        self.values.get(key)?.get(idx)?.as_i64()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct BatchLocationRecord {
    pub(crate) location: String,
    #[serde(default)]
    pub(crate) country: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct BatchLocationInput {
    pub(crate) location: String,
    pub(crate) country: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchSignalEnvelope {
    pub(crate) source: String,
    pub(crate) fetched_at: DateTime<Utc>,
    pub(crate) profile: String,
    pub(crate) items: Vec<BatchSignalItem>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchSignalItem {
    pub(crate) input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signal: Option<crate::signals::SignalEnvelope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_saved_places() {
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
}
