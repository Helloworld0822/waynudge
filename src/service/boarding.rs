use crate::{
    error::{Error, Result},
    routines::Routine,
    routing::Comparison,
    service::local,
};
use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::Serialize;
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct BoardingAdvice {
    pub station_name: String,
    pub line_id: Option<String>,
    pub planned_board_at: String,
    pub next_observed_train_at: Option<String>,
    pub note: String,
}
pub(super) fn advice(
    comparison: &Comparison,
    routine: &Routine,
    leave: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<BoardingAdvice>> {
    let Some(boarding) = comparison
        .selected
        .as_ref()
        .and_then(|route| route.subway_boarding.as_ref())
    else {
        return Ok(None);
    };
    let Some(leave) = leave else { return Ok(None) };
    let leave = DateTime::parse_from_rfc3339(leave)
        .map_err(|_| Error::Provider)?
        .with_timezone(&Utc);
    let access = std::time::Duration::try_from_secs_f64(
        f64::from(boarding.access_minutes) * routine.walking_factor * 60.0,
    )
    .map_err(|_| Error::Provider)?;
    let access = Duration::from_std(access).map_err(|_| Error::Provider)?;
    let next = boarding
        .live
        .as_ref()
        .into_iter()
        .flat_map(|live| live.arrivals.iter())
        .filter(|row| row.fresh)
        .filter_map(|row| {
            row.arrival_seconds
                .map(|seconds| row.reported_at + Duration::seconds(i64::from(seconds)))
        })
        .filter(|at| *at >= now + access)
        .min()
        .map(local)
        .transpose()?;
    Ok(Some(BoardingAdvice {station_name:boarding.station_name.clone(),line_id:boarding.line_id.clone(),planned_board_at:local(leave+access)?,next_observed_train_at:next,note:"planned_board_at is an estimate from scheduled route and access time, not a timetable guarantee. Observed trains may terminate before your destination; verify destination and platform.".into()}))
}
