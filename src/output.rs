use crate::{
    cache::CacheStatus,
    cli::{Cli, OutputFormat},
    models::{BatchSignalEnvelope, ForecastEnvelope, Location, Series},
    signals::{DemandFlags, SignalEnvelope, SummaryEnvelope, ThresholdEnvelope},
};
use anyhow::{Result, anyhow};
use comfy_table::{Table, presets::UTF8_FULL};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) enum TableData<'a> {
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

pub(crate) fn render<T: Serialize>(cli: &Cli, value: &T, table_data: TableData<'_>) -> Result<()> {
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
                "Input",
                "Location",
                "Country Code",
                "Requested Country",
                "Date",
                "Max C",
                "Min C",
                "Rain %",
                "Rain mm",
                "Flags",
                "Error",
            ]);
            for item in &batch.items {
                if let Some(signal) = &item.signal {
                    for day in &signal.days {
                        table.add_row([
                            item.input.clone(),
                            signal.location.name.clone(),
                            signal.location.country_code.clone().unwrap_or_default(),
                            item.requested_country.clone().unwrap_or_default(),
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
                        String::new(),
                        item.requested_country.clone().unwrap_or_default(),
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
                "requested_country",
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
                            item.requested_country.as_deref().unwrap_or(""),
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
                        "",
                        item.requested_country.as_deref().unwrap_or(""),
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
        let mut row = vec![series.time.get(idx).cloned().unwrap_or_default()];
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
        let mut row = vec![series.time.get(idx).cloned().unwrap_or_default()];
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
