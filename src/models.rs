use crate::cache::CacheState;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

static CONFIG_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_CONFIG_SYMLINK_HOPS: usize = 32;

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
        let save_path = config_save_path(path)?;
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        let temp_path = config_temp_path(&save_path);
        fs::write(&temp_path, text)
            .with_context(|| format!("failed to write temporary config {}", temp_path.display()))?;
        if let Ok(metadata) = fs::metadata(&save_path) {
            fs::set_permissions(&temp_path, metadata.permissions()).with_context(|| {
                format!(
                    "failed to preserve config permissions on {}",
                    temp_path.display()
                )
            })?;
        }
        #[cfg(windows)]
        if save_path.exists() {
            fs::remove_file(&save_path)
                .with_context(|| format!("failed to replace config {}", save_path.display()))?;
        }
        if let Err(error) = fs::rename(&temp_path, &save_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error)
                .with_context(|| format!("failed to commit config {}", save_path.display()));
        }
        Ok(())
    }
}

fn config_save_path(path: &Path) -> Result<PathBuf> {
    let mut current = path.to_path_buf();
    for _ in 0..MAX_CONFIG_SYMLINK_HOPS {
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = fs::read_link(&current).with_context(|| {
                    format!("failed to read config symlink {}", current.display())
                })?;
                current = if target.is_absolute() {
                    target
                } else {
                    current
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(target)
                };
            }
            Ok(_) | Err(_) => return Ok(current),
        }
    }

    Err(anyhow::anyhow!(
        "config symlink chain is too deep starting at {}",
        path.display()
    ))
}

fn config_temp_path(path: &Path) -> PathBuf {
    let counter = CONFIG_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", process::id(), counter))
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

    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};

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

    #[test]
    fn config_save_does_not_leave_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save(&path).unwrap();

        let temp_files = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();

        assert_eq!(temp_files, 0);
    }

    #[cfg(unix)]
    #[test]
    fn config_save_preserves_existing_file_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        Config::default().save(&path).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn config_save_preserves_symlink_and_updates_target() {
        let dir = tempfile::tempdir().unwrap();
        let target_path = dir.path().join("target.toml");
        let symlink_path = dir.path().join("config.toml");
        fs::write(&target_path, "").unwrap();
        symlink(&target_path, &symlink_path).unwrap();

        Config {
            places: BTreeMap::from([(
                "london".to_string(),
                Location {
                    id: None,
                    name: "London".to_string(),
                    country: None,
                    country_code: Some("GB".to_string()),
                    admin1: None,
                    latitude: 51.50853,
                    longitude: -0.12574,
                    timezone: None,
                    population: None,
                },
            )]),
        }
        .save(&symlink_path)
        .unwrap();

        assert!(
            fs::symlink_metadata(&symlink_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(fs::read_to_string(&target_path).unwrap().contains("london"));
    }

    #[cfg(unix)]
    #[test]
    fn config_save_bootstraps_dangling_symlink_target() {
        let dir = tempfile::tempdir().unwrap();
        let target_path = dir.path().join("target.toml");
        let symlink_path = dir.path().join("config.toml");
        symlink(&target_path, &symlink_path).unwrap();

        Config::default().save(&symlink_path).unwrap();

        assert!(
            fs::symlink_metadata(&symlink_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(target_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn config_save_preserves_symlink_chains_and_updates_final_target() {
        let dir = tempfile::tempdir().unwrap();
        let target_path = dir.path().join("target.toml");
        let middle_path = dir.path().join("middle.toml");
        let symlink_path = dir.path().join("config.toml");
        fs::write(&target_path, "").unwrap();
        symlink(&target_path, &middle_path).unwrap();
        symlink(&middle_path, &symlink_path).unwrap();

        Config::default().save(&symlink_path).unwrap();

        assert!(
            fs::symlink_metadata(&symlink_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(&middle_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::read_to_string(&target_path)
                .unwrap()
                .contains("[places]")
        );
    }
}
