mod calculation;
use crate::error::{Result, invalid};
pub use calculation::{PlanningPolicy, TravelContext, TravelScenario};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardingCheck {
    pub walk_minutes: u32,
    pub train_departure_after_seconds: u32,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulationRequest {
    /// User-supplied hypothetical percentages, never observations from a live API.
    pub congestion_percentages: Vec<f64>,
    pub context: TravelContext,
    pub depart_at: Option<DateTime<FixedOffset>>,
    pub boarding: Option<BoardingCheck>,
    /// Baseline duration for walking the entire trip, not just reaching the platform.
    pub walking_alternative_minutes: Option<u32>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct WalkingAlternative {
    pub duration_seconds: u32,
    pub arrival_at: Option<String>,
    pub meets_target: Option<bool>,
    pub faster_than_transit_in_scenario: Option<bool>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SimulationOutcome {
    pub congestion_percent: f64,
    pub calculation: TravelScenario,
    pub arrival_at_with_buffer: Option<String>,
    pub meets_target_in_scenario: Option<bool>,
    pub boarding_walk_seconds_with_buffer: Option<u32>,
    pub catches_train_in_scenario: Option<bool>,
    pub walking_alternative: Option<WalkingAlternative>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SimulationReport {
    pub source: String,
    pub realtime: bool,
    pub validated_prediction: bool,
    pub scenarios: Vec<SimulationOutcome>,
    pub warnings: Vec<String>,
}
pub fn simulate(input: SimulationRequest) -> Result<SimulationReport> {
    input.context.validate()?;
    if input.congestion_percentages.is_empty()
        || input.congestion_percentages.len() > 10
        || input
            .congestion_percentages
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=400.0).contains(p))
        || input
            .walking_alternative_minutes
            .is_some_and(|minutes| !(1..=1440).contains(&minutes))
        || input.boarding.as_ref().is_some_and(|b| {
            b.walk_minutes > input.context.station_walk_minutes
                || b.train_departure_after_seconds > 86400
        })
    {
        return Err(invalid(
            "simulation requires 1..10 congestion values in 0..400 and valid walking/boarding inputs",
        ));
    }
    let mut scenarios = vec![];
    for percent in &input.congestion_percentages {
        let calculation = input.context.calculate(*percent)?;
        let arrival = input
            .depart_at
            .map(|at| arrival(at, calculation.trip_seconds_with_buffer))
            .transpose()?;
        let boarding_walk_seconds = input
            .boarding
            .as_ref()
            .map(|b| {
                let baseline = calculation::seconds(
                    f64::from(b.walk_minutes) * 60.0 * input.context.walking_factor,
                )?;
                let load = ((*percent - input.context.policy.threshold_percent) / 100.0).max(0.0);
                let extra = calculation::seconds(
                    f64::from(baseline)
                        * input
                            .context
                            .policy
                            .extra_walk_ratio_per_100_percentage_points
                        * load,
                )?;
                baseline
                    .checked_add(extra)
                    .ok_or_else(|| invalid("boarding walk duration out of range"))
            })
            .transpose()?;
        let catches_train = boarding_walk_seconds
            .zip(input.boarding.as_ref())
            .map(|(seconds, b)| seconds <= b.train_departure_after_seconds);
        let arrival = arrival.filter(|_| catches_train != Some(false));
        let meets_target = arrival
            .zip(input.context.target_arrival)
            .map(|(at, target)| at <= target.with_timezone(&Utc));
        let walking_alternative = input
            .walking_alternative_minutes
            .map(|minutes| {
                let duration_seconds =
                    calculation::seconds(f64::from(minutes) * 60.0 * input.context.walking_factor)?;
                let at = input
                    .depart_at
                    .map(|start| self::arrival(start, duration_seconds))
                    .transpose()?;
                Ok::<_, crate::error::Error>(WalkingAlternative {
                    duration_seconds,
                    arrival_at: at.map(crate::service::local).transpose()?,
                    meets_target: at
                        .zip(input.context.target_arrival)
                        .map(|(at, target)| at <= target.with_timezone(&Utc)),
                    faster_than_transit_in_scenario: (catches_train != Some(false))
                        .then_some(duration_seconds < calculation.trip_seconds_with_buffer),
                })
            })
            .transpose()?;
        scenarios.push(SimulationOutcome {
            congestion_percent: *percent,
            calculation,
            arrival_at_with_buffer: arrival.map(crate::service::local).transpose()?,
            meets_target_in_scenario: meets_target,
            boarding_walk_seconds_with_buffer: boarding_walk_seconds,
            catches_train_in_scenario: catches_train,
            walking_alternative,
        });
    }
    Ok(SimulationReport {source:"simulation".into(),realtime:false,validated_prediction:false,scenarios,
        warnings:vec!["Synthetic test only: congestion percentages and planning allowances are user inputs, not observed or trained predictions. No probabilities are calculated.".into(),
            "Train occupancy does not measure station pedestrian density. Walking slowdown and extra dwell time follow the explicit policy for sensitivity testing; they must not overwrite real route or delay data.".into(),
            "Boarding and arrival flags are arithmetic under these assumptions, not guarantees. If the train is missed, transit arrival and walking-vs-transit comparison are unknown because no next-train headway was supplied. Calculation durations exclude missed-train waiting.".into()]})
}
fn arrival(start: DateTime<FixedOffset>, seconds: u32) -> Result<DateTime<Utc>> {
    start
        .checked_add_signed(Duration::seconds(i64::from(seconds)))
        .map(|at| at.with_timezone(&Utc))
        .ok_or_else(|| invalid("simulation arrival out of range"))
}
#[cfg(test)]
#[path = "simulation/tests.rs"]
mod tests;
