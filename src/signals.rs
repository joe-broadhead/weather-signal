use crate::{
    cache::CacheState,
    cli::{SignalProfile, ThresholdArgs},
    models::{ForecastEnvelope, Location},
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct SignalEnvelope {
    pub(crate) source: String,
    pub(crate) location: Location,
    pub(crate) fetched_at: DateTime<Utc>,
    pub(crate) cache: CacheState,
    pub(crate) timezone: String,
    pub(crate) profile: String,
    pub(crate) days: Vec<DailySignal>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DailySignal {
    pub(crate) date: String,
    pub(crate) temp_max_c: Option<f64>,
    pub(crate) temp_min_c: Option<f64>,
    pub(crate) apparent_temp_max_c: Option<f64>,
    pub(crate) apparent_temp_min_c: Option<f64>,
    pub(crate) precipitation_mm: Option<f64>,
    pub(crate) precip_probability_max_pct: Option<i64>,
    pub(crate) precipitation_hours: Option<f64>,
    pub(crate) wind_speed_max_kmh: Option<f64>,
    pub(crate) wind_gust_max_kmh: Option<f64>,
    pub(crate) sunshine_hours: Option<f64>,
    pub(crate) uv_index_max: Option<f64>,
    pub(crate) weather_code: Option<i64>,
    pub(crate) flags: DemandFlags,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DemandFlags {
    pub(crate) rain_likely: bool,
    pub(crate) wet_day: bool,
    pub(crate) heavy_rain: bool,
    pub(crate) warm_day: bool,
    pub(crate) hot_day: bool,
    pub(crate) cold_day: bool,
    pub(crate) windy_day: bool,
    pub(crate) sunny_day: bool,
    pub(crate) high_uv: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ThresholdEnvelope {
    pub(crate) source: String,
    pub(crate) location: Location,
    pub(crate) fetched_at: DateTime<Utc>,
    pub(crate) cache: CacheState,
    pub(crate) timezone: String,
    pub(crate) criteria: ThresholdCriteria,
    pub(crate) matches: Vec<ThresholdMatch>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ThresholdCriteria {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rain_prob_gte: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) precip_mm_gte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temp_max_gte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temp_min_lte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) wind_gust_gte: Option<f64>,
}

impl ThresholdCriteria {
    pub(crate) fn from_args(args: &ThresholdArgs) -> Result<Self> {
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

    pub(crate) fn match_reasons(&self, day: &DailySignal) -> Vec<String> {
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
pub(crate) struct ThresholdMatch {
    pub(crate) date: String,
    pub(crate) reasons: Vec<String>,
    pub(crate) signal: DailySignal,
}

#[derive(Debug, Serialize)]
pub(crate) struct SummaryEnvelope {
    pub(crate) source: String,
    pub(crate) location: Location,
    pub(crate) fetched_at: DateTime<Utc>,
    pub(crate) cache: CacheState,
    pub(crate) timezone: String,
    pub(crate) profile: String,
    pub(crate) headline: String,
    pub(crate) total_days: usize,
    pub(crate) risk_days: usize,
    pub(crate) warm_days: usize,
    pub(crate) hot_days: usize,
    pub(crate) wet_days: usize,
    pub(crate) windy_days: usize,
    pub(crate) sunny_days: usize,
    pub(crate) days: Vec<DailySignal>,
}

impl SummaryEnvelope {
    pub(crate) fn from_signal(signal: SignalEnvelope) -> Self {
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
    pub(crate) fn from_forecast(
        envelope: ForecastEnvelope,
        profile: SignalProfile,
    ) -> Result<Self> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache::CacheState, models::Series};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn signal_thresholds_match_plan() {
        let signal =
            SignalEnvelope::from_forecast(sample_forecast_envelope(), SignalProfile::Demand)
                .unwrap();
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
}
