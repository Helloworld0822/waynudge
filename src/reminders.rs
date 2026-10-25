use crate::{
    error::{Result, invalid},
    routines::{Routine, text, time},
    service::{Service, seoul},
};
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderRequest {
    pub routine_id: String,
    pub notification_time: String,
    pub weekdays: Vec<u32>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReminderPlan {
    pub title: String,
    pub timezone: String,
    pub notification_time: String,
    pub weekdays: Vec<u32>,
    pub schedule_ical: String,
    pub prompt: String,
    pub created: bool,
    pub delivery: String,
}
impl Service {
    pub async fn prepare_reminder(
        &self,
        user: &str,
        input: ReminderRequest,
    ) -> Result<ReminderPlan> {
        text(&input.routine_id)?;
        let routine = self.store.get(user, &input.routine_id).await?;
        build(&routine, &input, Utc::now())
    }
}
fn build(routine: &Routine, input: &ReminderRequest, now: DateTime<Utc>) -> Result<ReminderPlan> {
    let at = time(&input.notification_time)?;
    if input.weekdays.is_empty()
        || input.weekdays.len() > 7
        || input.weekdays.iter().any(|day| !(1..=7).contains(day))
    {
        return Err(invalid(
            "reminder weekdays must be Monday=1 through Sunday=7",
        ));
    }
    let days: BTreeSet<u32> = input.weekdays.iter().copied().collect();
    let offset = seoul()?;
    let today = now.with_timezone(&offset).date_naive();
    let mut start = None;
    for next in 0..=7 {
        let day = today
            .checked_add_signed(Duration::days(next))
            .ok_or_else(|| invalid("date out of range"))?;
        let candidate = offset
            .from_local_datetime(&day.and_time(at))
            .single()
            .ok_or_else(|| invalid("invalid local reminder time"))?;
        if candidate.with_timezone(&Utc) > now && days.contains(&day.weekday().number_from_monday())
        {
            start = Some(candidate);
            break;
        }
    }
    let start = start.ok_or_else(|| invalid("no next reminder occurrence"))?;
    let names = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
    let byday = days
        .iter()
        .map(|day| {
            names
                .get(usize::try_from(day.saturating_sub(1)).unwrap_or(7))
                .copied()
                .ok_or_else(|| invalid("invalid weekday"))
        })
        .collect::<Result<Vec<_>>>()?
        .join(",");
    let schedule = format!(
        "BEGIN:VEVENT\nDTSTART;TZID=Asia/Seoul:{}\nRRULE:FREQ=WEEKLY;BYDAY={};BYHOUR={};BYMINUTE={};BYSECOND=0\nEND:VEVENT",
        start.format("%Y%m%dT%H%M%S"),
        byday,
        at.hour(),
        at.minute()
    );
    let id = serde_json::to_string(&routine.id)?;
    let prompt = format!(
        "Asia/Seoul 기준 오늘의 출퇴근 알림. 대한민국 공휴일과 대체공휴일은 제외한다. 공휴일 여부는 사용 가능한 신뢰할 수 있는 달력이나 공식 정보로 확인한다. routine_id={id}에 get_commute_brief를 호출한다. active=false이면 알림을 생략한다. 최신 응답의 prepare_at, leave_at, boarding.station_name, boarding.planned_board_at을 사용해 준비 시각, 집에서 나갈 시각, 어느 역에서 몇 시에 지하철을 타야 하는지 한국어로 간결하게 알린다. next_observed_train_at은 실제 관측 도착예정으로 구분하고 탑승 보장을 하지 않는다. source=mock이면 모의 데이터임을 명시한다. 혼잡도는 제공된 실제 데이터가 있을 때만 표시한다. API 실패나 정보 부족을 숨기거나 숫자를 만들지 않는다. 알림 전달은 호스트 에이전트의 예약 알림 기능만 사용한다."
    );
    Ok(ReminderPlan {
        title: format!("{} 출퇴근 알림", routine.name),
        timezone: "Asia/Seoul".into(),
        notification_time: input.notification_time.clone(),
        weekdays: days.into_iter().collect(),
        schedule_ical: schedule,
        prompt,
        created: false,
        delivery: "host_scheduler".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn next_occurrence_skips_past_time_and_uses_seoul_weekdays() -> Result<()> {
        // Given
        #[derive(Deserialize)]
        struct Wrapped {
            routine: Routine,
        }
        let mut routine =
            serde_json::from_str::<Wrapped>(include_str!("../examples/routine.json"))?.routine;
        routine.id = "test".into();
        let input = ReminderRequest {
            routine_id: "test".into(),
            notification_time: "07:30".into(),
            weekdays: vec![1, 2, 3, 4, 5],
        };
        // When
        let plan = build(
            &routine,
            &input,
            "2026-10-25T00:00:00Z"
                .parse()
                .map_err(|_| invalid("test timestamp"))?,
        )?;
        // Then
        assert!(
            plan.schedule_ical
                .contains("DTSTART;TZID=Asia/Seoul:20261026T073000")
        );
        assert!(plan.schedule_ical.contains("BYDAY=MO,TU,WE,TH,FR"));
        assert!(!plan.created);
        Ok(())
    }
}
