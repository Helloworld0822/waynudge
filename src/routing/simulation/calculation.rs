use crate::error::{Result, invalid};
use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningPolicy {
    pub threshold_percent: f64,
    pub extra_walk_ratio_per_100_percentage_points: f64,
    pub extra_dwell_seconds_per_stop_per_100_percentage_points: f64,
}
impl Default for PlanningPolicy {
    fn default() -> Self {
        // Planning allowances, not coefficients learned from observed congestion and delays.
        Self {
            threshold_percent: 100.0,
            extra_walk_ratio_per_100_percentage_points: 0.25,
            extra_dwell_seconds_per_stop_per_100_percentage_points: 10.0,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TravelContext {
    /// Unadjusted route duration; includes station_walk_minutes but excludes this scenario buffer.
    pub base_trip_minutes: u32,
    /// Only station access/transfer walking, not outdoor walking. Must be supplied from a verified route.
    pub station_walk_minutes: u32,
    pub remaining_train_stops: u32,
    #[serde(default = "one")]
    pub walking_factor: f64,
    #[serde(default)]
    pub policy: PlanningPolicy,
    pub target_arrival: Option<DateTime<FixedOffset>>,
    #[serde(default)]
    pub buffer_minutes: u32,
}
fn one() -> f64 {
    1.0
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TravelScenario {
    pub basis: String,
    pub validated_prediction: bool,
    pub context: TravelContext,
    pub baseline_walk_seconds: u32,
    pub walking_buffer_seconds: u32,
    pub walk_seconds_with_buffer: u32,
    pub train_dwell_buffer_seconds: u32,
    pub baseline_trip_seconds: u32,
    pub total_buffer_seconds: u32,
    pub trip_seconds_with_buffer: u32,
    pub leave_at_with_buffer: Option<String>,
}
impl TravelContext {
    pub(super) fn validate(&self) -> Result<()> {
        let p = &self.policy;
        if !(1..=1440).contains(&self.base_trip_minutes)
            || self.station_walk_minutes > self.base_trip_minutes
            || self.station_walk_minutes > 180
            || self.remaining_train_stops > 200
            || self.buffer_minutes > 180
            || !self.walking_factor.is_finite()
            || !(0.5..=3.0).contains(&self.walking_factor)
            || !p.threshold_percent.is_finite()
            || !(1.0..=200.0).contains(&p.threshold_percent)
            || !p.extra_walk_ratio_per_100_percentage_points.is_finite()
            || !(0.0..=2.0).contains(&p.extra_walk_ratio_per_100_percentage_points)
            || !p
                .extra_dwell_seconds_per_stop_per_100_percentage_points
                .is_finite()
            || !(0.0..=120.0).contains(&p.extra_dwell_seconds_per_stop_per_100_percentage_points)
        {
            return Err(invalid(
                "invalid congestion travel context or planning policy",
            ));
        }
        Ok(())
    }
    pub(super) fn calculate(&self, percent: f64) -> Result<TravelScenario> {
        self.validate()?;
        if !percent.is_finite() || percent < 0.0 {
            return Err(invalid(
                "congestion percentage must be finite and nonnegative",
            ));
        }
        let load = ((percent - self.policy.threshold_percent) / 100.0).max(0.0);
        let baseline_walk_seconds =
            seconds(f64::from(self.station_walk_minutes) * 60.0 * self.walking_factor)?;
        let walking_buffer_seconds = seconds(
            f64::from(baseline_walk_seconds)
                * self.policy.extra_walk_ratio_per_100_percentage_points
                * load,
        )?;
        let train_dwell_buffer_seconds = seconds(
            f64::from(self.remaining_train_stops)
                * self
                    .policy
                    .extra_dwell_seconds_per_stop_per_100_percentage_points
                * load,
        )?;
        let total_buffer_seconds = walking_buffer_seconds
            .checked_add(train_dwell_buffer_seconds)
            .ok_or_else(|| invalid("congestion buffer too large"))?;
        let baseline_trip_seconds =
            (self.base_trip_minutes - self.station_walk_minutes) * 60 + baseline_walk_seconds;
        let trip_seconds_with_buffer = baseline_trip_seconds
            .checked_add(total_buffer_seconds)
            .ok_or_else(|| invalid("congestion trip time too large"))?;
        let leave_at_with_buffer = self
            .target_arrival
            .map(|target| {
                target
                    .checked_sub_signed(chrono::Duration::seconds(
                        i64::from(trip_seconds_with_buffer) + i64::from(self.buffer_minutes) * 60,
                    ))
                    .ok_or_else(|| invalid("arrival time out of range"))
                    .and_then(|time| crate::service::local(time.with_timezone(&chrono::Utc)))
            })
            .transpose()?;
        Ok(TravelScenario {
            basis: "unvalidated_planning_policy_v1".into(),
            validated_prediction: false,
            context: self.clone(),
            baseline_walk_seconds,
            walking_buffer_seconds,
            walk_seconds_with_buffer: baseline_walk_seconds
                .checked_add(walking_buffer_seconds)
                .ok_or_else(|| invalid("walking time too large"))?,
            train_dwell_buffer_seconds,
            baseline_trip_seconds,
            total_buffer_seconds,
            trip_seconds_with_buffer,
            leave_at_with_buffer,
        })
    }
}
pub(super) fn seconds(value: f64) -> Result<u32> {
    let duration = std::time::Duration::try_from_secs_f64(value)
        .map_err(|_| invalid("congestion duration out of range"))?;
    u32::try_from(duration.as_nanos().div_ceil(1_000_000_000))
        .map_err(|_| invalid("congestion duration out of range"))
}
#[cfg(test)]
#[path = "calculation_tests.rs"]
mod tests;
