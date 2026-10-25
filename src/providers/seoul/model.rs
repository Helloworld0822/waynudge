use crate::error::{Error, Result, invalid};
use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Up,
    Down,
}
impl Direction {
    fn matches(self, value: &str) -> bool {
        match self {
            Self::Up => matches!(value, "상행" | "내선"),
            Self::Down => matches!(value, "하행" | "외선"),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubwayQuery {
    pub station_name: String,
    pub line_id: Option<String>,
    pub direction: Option<Direction>,
}
impl SubwayQuery {
    pub fn validate(&self) -> Result<()> {
        if self.station_name.trim().is_empty()
            || self.station_name.len() > 160
            || self
                .station_name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | '?' | '#' | '%'))
        {
            return Err(invalid("invalid subway station name"));
        }
        if self
            .line_id
            .as_ref()
            .is_some_and(|id| id.len() != 4 || !id.bytes().all(|c| c.is_ascii_digit()))
        {
            return Err(invalid("line_id must be Seoul's four-digit subway ID"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SubwayArrival {
    pub station_name: String,
    pub line_id: String,
    pub direction: String,
    pub destination: String,
    pub train_number: String,
    pub arrival_seconds: Option<u32>,
    pub reported_at: DateTime<Utc>,
    pub fresh: bool,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SubwayArrivals {
    pub source: String,
    pub station_name: String,
    pub collected_at: DateTime<Utc>,
    pub arrivals: Vec<SubwayArrival>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SubwayBoarding {
    pub access_minutes: u32,
    pub access_walk_minutes: u32,
    #[serde(default)]
    pub access_walk_distance_meters: Option<f64>,
    pub station_name: String,
    pub line_id: Option<String>,
    pub direction: Option<Direction>,
    pub live: Option<SubwayArrivals>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Payload {
    pub error_message: Option<ApiStatus>,
    pub realtime_arrival_list: Option<Vec<RawArrival>>,
}
#[derive(Deserialize)]
pub(super) struct ApiStatus {
    pub code: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawArrival {
    subway_id: String,
    updn_line: String,
    statn_nm: String,
    bstatn_nm: String,
    btrain_no: String,
    barvl_dt: String,
    recptn_dt: String,
    arvl_msg2: String,
    arvl_cd: String,
}
pub(super) fn parse(
    payload: Payload,
    query: &SubwayQuery,
    now: DateTime<Utc>,
) -> Result<SubwayArrivals> {
    if payload
        .error_message
        .is_some_and(|status| status.code != "INFO-000")
    {
        return Err(Error::Provider);
    }
    let raw = payload.realtime_arrival_list.ok_or(Error::Provider)?;
    if raw.len() > 200 {
        return Err(Error::Provider);
    }
    let offset = FixedOffset::east_opt(9 * 3600).ok_or(Error::Provider)?;
    let mut arrivals = Vec::new();
    let mut warnings = Vec::new();
    for row in raw {
        if normalize(&row.statn_nm) != normalize(&query.station_name)
            || query
                .line_id
                .as_ref()
                .is_some_and(|id| id != &row.subway_id)
            || query
                .direction
                .is_some_and(|direction| !direction.matches(&row.updn_line))
            || row.arvl_cd == "2"
        {
            continue;
        }
        let Some(reported_at) = NaiveDateTime::parse_from_str(&row.recptn_dt, "%Y-%m-%d %H:%M:%S")
            .ok()
            .and_then(|at| offset.from_local_datetime(&at).single())
            .map(|at| at.with_timezone(&Utc))
        else {
            warnings.push("Invalid observation timestamp; row omitted.".into());
            continue;
        };
        let age = now.signed_duration_since(reported_at).num_seconds();
        let fresh = (-30..=120).contains(&age);
        let seconds = row
            .barvl_dt
            .parse::<u32>()
            .ok()
            .filter(|seconds| *seconds <= 86_400);
        let seconds =
            seconds.filter(|seconds| *seconds > 0 || matches!(row.arvl_cd.as_str(), "0" | "1"));
        if !fresh {
            warnings
                .push("Some subway observations are stale; do not treat them as current.".into());
        }
        arrivals.push(SubwayArrival {
            station_name: row.statn_nm,
            line_id: row.subway_id,
            direction: row.updn_line,
            destination: row.bstatn_nm,
            train_number: row.btrain_no,
            arrival_seconds: seconds,
            reported_at,
            fresh,
            message: row.arvl_msg2,
        });
    }
    arrivals.sort_by_key(|row| {
        (
            !row.fresh,
            row.arrival_seconds.unwrap_or(u32::MAX),
            row.line_id.clone(),
            row.train_number.clone(),
        )
    });
    warnings.push("Arrival observations are not a measured whole-route delay; scheduled journey time is unchanged.".into());
    if arrivals.is_empty() {
        warnings.push(
            "No matching train observations; this does not prove a service suspension.".into(),
        );
    }
    warnings.sort();
    warnings.dedup();
    Ok(SubwayArrivals {
        source: "seoul_subway".into(),
        station_name: query.station_name.clone(),
        collected_at: now,
        arrivals,
        warnings,
    })
}
pub(super) fn normalize(station: &str) -> &str {
    let station = station.trim();
    station.strip_suffix('역').unwrap_or(station)
}
