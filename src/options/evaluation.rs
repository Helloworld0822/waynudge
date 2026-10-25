use crate::{
    error::{Result, invalid},
    routing::simulation::{PlanningPolicy, SimulationRequest, TravelContext, simulate},
};

use super::{TravelOption, TravelOptionCandidate, TravelOptionsRequest};

impl TravelOption {
    pub(super) fn evaluate(
        &self,
        request: &TravelOptionsRequest,
        congestion_percent: f64,
    ) -> Result<TravelOptionCandidate> {
        let bus = self.bus_at(request)?;
        let road = self.road_at(request.depart_at);
        let road_delay_seconds = road.map_or(0, |(_, seconds)| seconds);
        let speed = request
            .walking_speed_kmh
            .filter(|_| !self.missing_station_distance());
        let station_minutes = corrected_minutes(
            self.station_walk_distance_meters,
            self.station_walk_minutes,
            speed,
        )?;
        let walk_minutes = if self
            .walk_distance_meters
            .is_none_or(|distance| distance == 0.0)
        {
            (self.walk_minutes - self.station_walk_minutes)
                .checked_add(station_minutes)
                .ok_or_else(|| invalid("corrected walking duration out of range"))?
        } else {
            corrected_minutes(self.walk_distance_meters, self.walk_minutes, speed)?
        };
        if station_minutes > walk_minutes {
            return Err(invalid(
                "corrected station walking exceeds total walking; provide consistent segment distances",
            ));
        }
        let base_trip_minutes = (self.total_minutes - self.walk_minutes)
            .checked_add(walk_minutes)
            .ok_or_else(|| invalid("corrected trip duration out of range"))?;
        let report = simulate(SimulationRequest {
            congestion_percentages: vec![congestion_percent],
            context: TravelContext {
                base_trip_minutes,
                station_walk_minutes: station_minutes,
                remaining_train_stops: self.remaining_train_stops,
                walking_factor: 1.0,
                policy: PlanningPolicy::default(),
                target_arrival: request.target_arrival,
                buffer_minutes: 0,
            },
            depart_at: request.depart_at,
            boarding: None,
            walking_alternative_minutes: None,
        })?;
        let scenario = report
            .scenarios
            .into_iter()
            .next()
            .ok_or_else(|| invalid("simulation did not return the requested scenario"))?;
        let total_seconds = scenario
            .calculation
            .trip_seconds_with_buffer
            .checked_add(road_delay_seconds)
            .ok_or_else(|| invalid("road-adjusted trip duration out of range"))?;
        let arrival = request
            .depart_at
            .map(|start| {
                start
                    .checked_add_signed(chrono::Duration::seconds(i64::from(total_seconds)))
                    .ok_or_else(|| invalid("road-adjusted arrival out of range"))
            })
            .transpose()?;
        let (cost_won, fare_source) = self.fare(request.fare_profile);
        Ok(TravelOptionCandidate {
            id: self.id.clone(),
            label: self.label.clone(),
            mode: self.mode,
            total_seconds,
            road_delay_seconds,
            applied_bus_observation: bus.cloned(),
            applied_road_traffic: road.map(|(traffic, _)| traffic.clone()),
            fare_profile: request.fare_profile,
            fare_source,
            fare_verified: false,
            walk_seconds: walk_minutes * 60 + scenario.calculation.walking_buffer_seconds,
            walking_buffer_seconds: scenario.calculation.walking_buffer_seconds,
            train_dwell_buffer_seconds: scenario.calculation.train_dwell_buffer_seconds,
            transfers: self.transfers,
            cost_won,
            arrival_at: arrival
                .map(|at| crate::service::local(at.with_timezone(&chrono::Utc)))
                .transpose()?,
            meets_target: arrival
                .zip(request.target_arrival)
                .map(|(arrival, target)| arrival <= target),
        })
    }

    pub(super) fn calibration_warning(&self, speed: Option<f64>) -> Option<String> {
        speed?;
        if self.missing_station_distance() {
            return Some(format!(
                "{}: station walking distance is missing; all walking retains the supplied baseline because the station portion cannot be split safely.",
                self.id
            ));
        }
        (self.walk_minutes > 0 && self.walk_distance_meters.is_none_or(|distance| distance == 0.0))
            .then(|| format!("{}: total walking distance is missing; only any known station segment is corrected, and other walking retains its supplied baseline.", self.id))
    }

    fn missing_station_distance(&self) -> bool {
        self.station_walk_minutes > 0
            && self
                .station_walk_distance_meters
                .is_none_or(|distance| distance == 0.0)
    }
}

fn corrected_minutes(distance: Option<f64>, baseline: u32, speed: Option<f64>) -> Result<u32> {
    match (speed, distance) {
        (Some(speed), Some(distance)) if distance > 0.0 || baseline == 0 => {
            let duration = std::time::Duration::try_from_secs_f64(distance / (speed / 3.6))
                .map_err(|_| invalid("corrected walking duration out of range"))?;
            u32::try_from(duration.as_nanos().div_ceil(60_000_000_000))
                .map_err(|_| invalid("corrected walking duration out of range"))
        }
        _ => Ok(baseline),
    }
}
