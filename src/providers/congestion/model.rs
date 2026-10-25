use crate::providers::seoul::Direction;
use crate::{
    error::{Result, invalid},
    routines::{text, time},
};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DayType {
    Weekday,
    Saturday,
    Sunday,
}
impl DayType {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Weekday => "평일",
            Self::Saturday => "토요일",
            Self::Sunday => "일요일",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CongestionQuery {
    pub station_name: String,
    pub line: u8,
    pub day_type: DayType,
    pub time: String,
    pub direction: Option<Direction>,
}
impl CongestionQuery {
    pub fn validate(&self) -> Result<()> {
        text(&self.station_name)?;
        if self.station_name.trim().is_empty()
            || self.station_name.len() > 120
            || !(1..=8).contains(&self.line)
        {
            return Err(invalid(
                "subway congestion requires a station and Seoul Metro line 1..8",
            ));
        }
        time(&self.time)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct CongestionObservation {
    pub station_name: String,
    pub station_code: String,
    pub line: u8,
    pub day_type: DayType,
    pub direction: String,
    pub congestion_percent: Option<f64>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SubwayCongestionReport {
    pub source: String,
    pub source_url: String,
    pub realtime: bool,
    pub dataset_as_of: String,
    pub served_at: DateTime<Utc>,
    pub time_slot_start: String,
    pub time_slot_end: String,
    pub observations: Vec<CongestionObservation>,
    pub warnings: Vec<String>,
}
