use super::{CongestionQuery, DATASET_AS_OF, SubwayCongestionReport, model::CongestionObservation};
use crate::{
    error::{Error, Result},
    providers::seoul::Direction,
};
use chrono::{Timelike, Utc};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(untagged)]
enum Cell {
    Text(String),
    Number(f64),
}
impl Cell {
    fn text(&self) -> String {
        match self {
            Self::Text(v) => v.trim().to_string(),
            Self::Number(v) => v.to_string(),
        }
    }
    fn percent(&self) -> Result<Option<f64>> {
        let value = self.text();
        if value.is_empty() || matches!(value.as_str(), "-" | "N/A") {
            return Ok(None);
        }
        let value: f64 = value.parse().map_err(|_| Error::Provider)?;
        if !value.is_finite() {
            return Err(Error::Provider);
        }
        Ok((value >= 0.0).then_some(value))
    }
}
#[derive(Deserialize)]
struct Row {
    #[serde(rename = "구분")]
    day: String,
    #[serde(rename = "호선")]
    line: Cell,
    #[serde(rename = "역번호")]
    station_code: Cell,
    #[serde(rename = "역명")]
    station: String,
    #[serde(rename = "상하구분")]
    direction: String,
    #[serde(flatten)]
    slots: HashMap<String, Option<Cell>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    data: Vec<Row>,
    current_count: usize,
    match_count: usize,
    total_count: usize,
    page: u32,
}
fn station(value: &str) -> &str {
    let value = value.trim();
    value.strip_suffix('역').unwrap_or(value)
}
pub(super) fn parse(bytes: &[u8], query: &CongestionQuery) -> Result<SubwayCongestionReport> {
    query.validate()?;
    let payload: Payload = serde_json::from_slice(bytes).map_err(|_| Error::Provider)?;
    if payload.page != 1
        || payload.data.len() > 2000
        || payload.current_count != payload.data.len()
        || payload.match_count != payload.data.len()
        || payload.total_count != payload.data.len()
    {
        return Err(Error::Provider);
    }
    let time = crate::routines::time(&query.time)?;
    let minute = time.minute() / 30 * 30;
    let slot = if time.hour() == 0 {
        format!("00시{minute:02}분")
    } else {
        format!("{}시{minute:02}분", time.hour())
    };
    let start_minutes = time.hour() * 60 + minute;
    let end_minutes = (start_minutes + 30) % 1440;
    let mut observations = vec![];
    for row in payload.data {
        let line: u8 = row
            .line
            .text()
            .trim_end_matches("호선")
            .parse()
            .map_err(|_| Error::Provider)?;
        let direction_matches = query.direction.is_none_or(|direction| match direction {
            Direction::Up => matches!(row.direction.as_str(), "상선" | "상행" | "내선"),
            Direction::Down => matches!(row.direction.as_str(), "하선" | "하행" | "외선"),
        });
        if station(&row.station) != station(&query.station_name)
            || line != query.line
            || row.day != query.day_type.label()
            || !direction_matches
        {
            continue;
        }
        let percent = row
            .slots
            .get(&slot)
            .and_then(Option::as_ref)
            .map(Cell::percent)
            .transpose()?
            .flatten();
        observations.push(CongestionObservation {
            station_name: row.station,
            station_code: row.station_code.text(),
            line,
            day_type: query.day_type,
            direction: row.direction,
            congestion_percent: percent,
        });
    }
    let mut warnings = vec!["Historical 30-minute averages, not live train or car occupancy. Events and service disruptions are not reflected; do not use this as a live delay or boarding guarantee.".into()];
    if observations.is_empty()
        || observations
            .iter()
            .any(|row| row.congestion_percent.is_none())
    {
        warnings.push("Congestion data is unavailable for some or all requested station, direction and time slots; missing does not mean uncrowded.".into());
    }
    Ok(SubwayCongestionReport {
        source: "seoul_metro_odcloud".into(),
        source_url: "https://www.data.go.kr/data/15071311/fileData.do".into(),
        realtime: false,
        dataset_as_of: DATASET_AS_OF.into(),
        served_at: Utc::now(),
        time_slot_start: format!("{:02}:{minute:02}", time.hour()),
        time_slot_end: format!("{:02}:{:02}", end_minutes / 60, end_minutes % 60),
        observations,
        warnings,
    })
}
