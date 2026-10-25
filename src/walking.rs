use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    error::{Result, invalid, private_access},
    routines::{Place, text},
    service::Service,
};

mod storage;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalkingDistanceSource {
    UserMeasured,
    MapWalkingRoute,
}

impl WalkingDistanceSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::UserMeasured => "user_measured",
            Self::MapWalkingRoute => "map_walking_route",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WalkingObservation {
    pub routine_id: String,
    pub observation_id: String,
    pub origin: Place,
    pub destination: Place,
    pub distance_meters: f64,
    pub elapsed_minutes: f64,
    pub distance_source: WalkingDistanceSource,
    pub confirmed: bool,
}

impl WalkingObservation {
    fn measured_speed(&self) -> Result<f64> {
        text(&self.routine_id)?;
        text(&self.observation_id)?;
        self.origin.validate()?;
        self.destination.validate()?;
        if !self.distance_meters.is_finite()
            || self.distance_meters <= 0.0
            || self.distance_meters > 100_000.0
            || !self.elapsed_minutes.is_finite()
            || self.elapsed_minutes <= 0.0
            || self.elapsed_minutes > 1440.0
        {
            return Err(invalid("walking distance or elapsed time out of range"));
        }
        let speed = self.distance_meters * 0.06 / self.elapsed_minutes;
        if !speed.is_finite() || !(0.5..=12.0).contains(&speed) {
            return Err(invalid("measured walking speed must be 0.5..12 km/h"));
        }
        Ok(speed)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct WalkingCalibration {
    pub saved: bool,
    pub measured_speed_kmh: f64,
    pub profile_speed_kmh: Option<f64>,
    pub sample_count: Option<u32>,
    pub warnings: Vec<String>,
}

impl Service {
    pub async fn record_walking_observation(
        &self,
        user: &str,
        input: WalkingObservation,
    ) -> Result<WalkingCalibration> {
        let measured_speed_kmh = input.measured_speed()?;
        let mut output = WalkingCalibration {
            saved: false,
            measured_speed_kmh,
            profile_speed_kmh: None,
            sample_count: None,
            warnings: vec![
                "Distance is caller-supplied and must follow the actual walking route, not a straight line.".into(),
                "Only confirmed observations are saved; no continuous location tracking is performed.".into(),
            ],
        };
        if input.confirmed {
            private_access(user)?;
            text(user)?;
            let (speed, count) = self.store.save_walking_observation(user, &input).await?;
            output.saved = true;
            output.profile_speed_kmh = Some(speed);
            output.sample_count = Some(count);
        }
        Ok(output)
    }
}
