use anyhow::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tracing::debug;

const SCHEMA_VERSION: &str = "v1";
static CACHE_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CacheState {
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
pub(crate) struct Cache {
    pub(crate) dir: PathBuf,
}

impl Cache {
    pub(crate) fn new(dir: PathBuf) -> Result<Self> {
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

    pub(crate) fn get(&self, url: &str, ttl: Duration) -> Result<Option<Value>> {
        let path = self.key(url);
        if !path.exists() {
            debug!(url, "cache file missing");
            return Ok(None);
        }
        let modified = fs::metadata(&path)?.modified()?;
        if modified.elapsed().unwrap_or(Duration::MAX) > ttl {
            debug!(url, path = %path.display(), "cache file expired");
            return Ok(None);
        }
        debug!(url, path = %path.display(), "cache file read");
        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }

    pub(crate) fn put(&self, url: &str, value: &Value) -> Result<()> {
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
        debug!(url, "cache file written");
        Ok(())
    }

    fn temp_key(&self, path: &Path) -> PathBuf {
        let counter = CACHE_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.with_extension(format!("json.tmp.{}.{}", process::id(), counter))
    }

    pub(crate) fn status(&self) -> Result<CacheStatus> {
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

    pub(crate) fn clear(&self) -> Result<u64> {
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

    pub(crate) fn prune_older_than(&self, max_age: Duration) -> Result<u64> {
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

#[derive(serde::Serialize)]
pub(crate) struct CacheStatus {
    pub(crate) path: PathBuf,
    pub(crate) files: u64,
    pub(crate) bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
