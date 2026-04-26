use crate::APP_NAME;
use anyhow::{Context, Result, anyhow};
use chrono::NaiveDate;
use std::{env, path::PathBuf};

pub(crate) fn parse_lat_lon(input: &str) -> Option<(f64, f64)> {
    let (lat, lon) = input.split_once(',')?;
    let lat = lat.trim().parse().ok()?;
    let lon = lon.trim().parse().ok()?;
    Some((lat, lon))
}

pub(crate) fn validate_coordinates(lat: f64, lon: f64) -> Result<()> {
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

pub(crate) fn parse_date(input: &str, name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(input, "%Y-%m-%d")
        .with_context(|| format!("{name} date must use YYYY-MM-DD"))
}

pub(crate) fn default_config_path() -> PathBuf {
    xdg_home("XDG_CONFIG_HOME", ".config")
        .join(APP_NAME)
        .join("config.toml")
}

pub(crate) fn default_cache_dir() -> PathBuf {
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
}
