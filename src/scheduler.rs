use crate::{
    alerts::AlertDraft,
    error::{Result, invalid},
    routines::{Routine, time},
    service::{BriefRequest, Service, seoul},
};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use std::collections::HashMap;

impl Service {
    pub async fn evaluate_alerts(&self, user: &str, now: DateTime<Utc>) -> Result<u32> {
        let mut count = 0;
        for routine in self.store.list(user).await? {
            if let Some(day) = notification_day(&routine, now)? {
                count += u32::from(self.evaluate_routine(user, &routine, day, now).await?);
            }
        }
        Ok(count)
    }
    async fn evaluate_routine(
        &self,
        user: &str,
        routine: &Routine,
        day: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        let date = day.to_string();
        let brief = self
            .brief_at(
                user,
                BriefRequest {
                    routine_id: routine.id.clone(),
                    date: date.clone(),
                    current_route_id: routine.usual_route_id.clone(),
                },
                now,
            )
            .await?;
        if !brief.active {
            return Ok(false);
        }
        let kind = if brief.comparison.should_switch {
            "route_change"
        } else if brief
            .prepare_at
            .as_ref()
            .is_some_and(|at| DateTime::parse_from_rfc3339(at).is_ok_and(|at| now >= at))
        {
            "prepare"
        } else {
            return Ok(false);
        };
        let message = format!(
            "{}: prepare {}, leave {}. {} Source: {}. {}",
            routine.name,
            brief.prepare_at.as_deref().unwrap_or("unavailable"),
            brief.leave_at.as_deref().unwrap_or("unavailable"),
            brief.comparison.reason,
            brief.comparison.source,
            brief.comparison.warnings.join(" ")
        );
        self.store
            .enqueue_alert(
                user,
                &AlertDraft {
                    routine_id: routine.id.clone(),
                    date,
                    kind: kind.into(),
                    now,
                    message,
                    min_interval_minutes: routine.min_alert_interval_minutes,
                },
            )
            .await
    }
    pub async fn scheduler_tick(
        &self,
        checked: &mut HashMap<(String, String), DateTime<Utc>>,
    ) -> Result<()> {
        let now = Utc::now();
        checked.retain(|_, at| now.signed_duration_since(*at) < Duration::days(1));
        for (user, routine) in self.store.all_routines().await? {
            let Some(day) = notification_day(&routine, now)? else {
                continue;
            };
            let key = (user.clone(), routine.id.clone());
            if checked.get(&key).is_some_and(|at| {
                now.signed_duration_since(*at)
                    < Duration::minutes(i64::from(routine.min_alert_interval_minutes))
            }) {
                continue;
            }
            checked.insert(key, now);
            if self
                .evaluate_routine(&user, &routine, day, now)
                .await
                .is_err()
            {
                tracing::warn!("scheduled alert evaluation unavailable");
            }
        }
        Ok(())
    }
}
fn notification_day(routine: &Routine, now: DateTime<Utc>) -> Result<Option<NaiveDate>> {
    let offset = seoul()?;
    let today = now.with_timezone(&offset).date_naive();
    let tomorrow = today
        .succ_opt()
        .ok_or_else(|| invalid("date out of range"))?;
    for day in [today, tomorrow] {
        let target = offset
            .from_local_datetime(&day.and_time(time(&routine.arrival_time)?))
            .single()
            .ok_or_else(|| invalid("arrival out of range"))?
            .with_timezone(&Utc);
        let remaining = target.signed_duration_since(now);
        // shortcut: evaluate within six hours of arrival; longer journeys need a persisted horizon.
        if remaining >= Duration::zero() && remaining <= Duration::hours(6) {
            return Ok(Some(day));
        }
    }
    Ok(None)
}
