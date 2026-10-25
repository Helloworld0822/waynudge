use crate::{
    error::{Result, confirmed, invalid},
    providers::Provider,
    routines::{Place, Routine, date, text, time},
    routing::{self, CompareInput, Comparison},
    storage::{DayStatus, Feedback, Store},
};
use chrono::{DateTime, Datelike, Duration, FixedOffset, TimeZone, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone)]
pub struct Service {
    pub store: Store,
    pub provider: Provider,
    pub holidays: HashSet<String>,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpsertRequest {
    pub routine: Routine,
    pub confirmed: bool,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BriefRequest {
    pub routine_id: String,
    pub date: String,
    pub current_route_id: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TripStage {
    AtStation,
    Walking,
    TransferMissed,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplanRequest {
    pub routine_id: String,
    pub date: String,
    pub current_route_id: Option<String>,
    /// User-provided current station/location; no continuous tracking.
    pub current_location: Place,
    pub stage: TripStage,
    pub blocked_route_ids: Vec<String>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct RoutineList {
    pub routines: Vec<Routine>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MutationResult {
    pub saved: bool,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Brief {
    pub routine_id: String,
    pub date: String,
    pub active: bool,
    pub timezone: String,
    pub target_arrival: String,
    pub leave_at: Option<String>,
    pub prepare_at: Option<String>,
    pub estimated_arrival_if_leaving_now: Option<String>,
    pub late_if_leaving_now: bool,
    pub comparison: Comparison,
}
pub fn seoul() -> Result<FixedOffset> {
    FixedOffset::east_opt(9 * 3600).ok_or_else(|| invalid("timezone unavailable"))
}
impl Service {
    pub async fn list(&self, user: &str) -> Result<RoutineList> {
        Ok(RoutineList {
            routines: self.store.list(user).await?,
        })
    }
    pub async fn upsert(&self, user: &str, input: UpsertRequest) -> Result<MutationResult> {
        confirmed(input.confirmed)?;
        self.store.upsert(user, &input.routine).await?;
        Ok(MutationResult { saved: true })
    }
    pub async fn set_day(&self, user: &str, input: DayStatus) -> Result<MutationResult> {
        self.store.set_day(user, &input).await?;
        Ok(MutationResult { saved: true })
    }
    pub async fn feedback(&self, user: &str, input: Feedback) -> Result<MutationResult> {
        self.store.feedback(user, &input).await?;
        Ok(MutationResult { saved: true })
    }
    pub async fn brief(&self, user: &str, input: BriefRequest) -> Result<Brief> {
        self.brief_at(user, input, Utc::now()).await
    }
    pub async fn compare(&self, user: &str, input: BriefRequest) -> Result<Comparison> {
        Ok(self.brief(user, input).await?.comparison)
    }
    pub async fn brief_at(
        &self,
        user: &str,
        input: BriefRequest,
        now: DateTime<Utc>,
    ) -> Result<Brief> {
        text(&input.routine_id)?;
        let routine = self.store.get(user, &input.routine_id).await?;
        self.build(user, routine, input, now, None).await
    }
    pub async fn replan(&self, user: &str, input: ReplanRequest) -> Result<Brief> {
        input.current_location.validate()?;
        if input.blocked_route_ids.len() > 30 {
            return Err(invalid("at most 30 blocked routes"));
        }
        for id in &input.blocked_route_ids {
            text(id)?;
        }
        match input.stage {
            TripStage::TransferMissed if input.blocked_route_ids.is_empty() => {
                return Err(invalid("missed transfer requires blocked route IDs"));
            }
            TripStage::AtStation | TripStage::Walking | TripStage::TransferMissed => {}
        }
        let mut routine = self.store.get(user, &input.routine_id).await?;
        routine.origin = input.current_location;
        routine.preparation_minutes = 0;
        self.build(
            user,
            routine,
            BriefRequest {
                routine_id: input.routine_id,
                date: input.date,
                current_route_id: input.current_route_id,
            },
            Utc::now(),
            Some(input.blocked_route_ids),
        )
        .await
    }
    async fn build(
        &self,
        user: &str,
        routine: Routine,
        input: BriefRequest,
        now: DateTime<Utc>,
        blocked: Option<Vec<String>>,
    ) -> Result<Brief> {
        let day = date(&input.date)?;
        if let Some(id) = &input.current_route_id {
            text(id)?;
        }
        let target = seoul()?
            .from_local_datetime(&day.and_time(time(&routine.arrival_time)?))
            .single()
            .ok_or_else(|| invalid("invalid local arrival time"))?
            .with_timezone(&Utc);
        let active = self
            .store
            .day_enabled(user, &routine.id, &input.date)
            .await?
            .unwrap_or_else(|| {
                routine
                    .weekdays
                    .contains(&day.weekday().number_from_monday())
                    && !self.holidays.contains(&input.date)
            });
        let comparison = if active {
            let mut routes = self
                .provider
                .routes(&routine.origin, &routine.destination)
                .await?;
            if let Some(blocked) = blocked {
                routes.routes.retain(|route| !blocked.contains(&route.id));
            }
            routing::compare(&CompareInput {
                routine: routine.clone(),
                routes,
                current_route_id: input.current_route_id,
                now,
                target,
            })?
        } else {
            Comparison {
                selected: None,
                candidates: vec![],
                should_switch: false,
                reason: "Routine excluded for this date.".into(),
                warnings: vec![],
                source: "not_requested".into(),
            }
        };
        let (leave, prepare, arrival, late) = match &comparison.selected {
            Some(route) => {
                let duration = Duration::minutes(i64::from(
                    route.total_minutes.saturating_add(route.delay_minutes),
                ));
                let leave =
                    target - duration - Duration::minutes(i64::from(routine.buffer_minutes));
                let prepare = leave - Duration::minutes(i64::from(routine.preparation_minutes));
                let arrival = now + duration;
                (
                    Some(local(leave)?),
                    Some(local(prepare)?),
                    Some(local(arrival)?),
                    arrival > target,
                )
            }
            None => (None, None, None, false),
        };
        Ok(Brief {
            routine_id: routine.id,
            date: input.date,
            active,
            timezone: "Asia/Seoul".into(),
            target_arrival: local(target)?,
            leave_at: leave,
            prepare_at: prepare,
            estimated_arrival_if_leaving_now: arrival,
            late_if_leaving_now: late,
            comparison,
        })
    }
}
pub fn local(time: DateTime<Utc>) -> Result<String> {
    Ok(time.with_timezone(&seoul()?).to_rfc3339())
}
