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
        (f64::from(
            boarding
                .access_minutes
                .saturating_sub(boarding.access_walk_minutes),
        ) + f64::from(boarding.access_walk_minutes) * routine.walking_factor)
            * 60.0,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn walking_factor_does_not_slow_the_bus_access_leg() -> Result<()> {
        // Given: 15 minutes on a bus and 5 minutes walking before the subway.
        #[derive(serde::Deserialize)]
        struct Wrapped {
            routine: Routine,
        }
        let mut routine =
            serde_json::from_str::<Wrapped>(include_str!("../../examples/routine.json"))?.routine;
        routine.walking_factor = 2.0;
        let routes = crate::providers::Provider::mock()
            .routes(&routine.origin, &routine.destination)
            .await?;
        let mut route = routes
            .routes
            .into_iter()
            .find(|route| route.subway_boarding.is_some())
            .ok_or(Error::Provider)?;
        let boarding = route.subway_boarding.as_mut().ok_or(Error::Provider)?;
        boarding.access_minutes = 20;
        boarding.access_walk_minutes = 5;
        let comparison = Comparison {
            selected: Some(route),
            candidates: vec![],
            should_switch: false,
            reason: String::new(),
            warnings: vec![],
            source: "mock".into(),
        };
        // When
        let result = advice(
            &comparison,
            &routine,
            Some("2026-10-26T08:00:00+09:00"),
            Utc::now(),
        )?
        .ok_or(Error::Provider)?;
        // Then: 15 bus minutes + 10 walking minutes, not 40 minutes.
        assert_eq!(result.planned_board_at, "2026-10-26T08:25:00+09:00");
        Ok(())
    }
}
