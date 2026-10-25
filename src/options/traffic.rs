use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{TravelOption, TravelOptionsRequest};
use crate::{
    error::{Result, invalid},
    routines::{Mode, text},
};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Caller-supplied provenance; the server does not verify the provider.
    pub source: String,
    pub observed_at: DateTime<FixedOffset>,
    /// Valid for 1..=3600 seconds after observed_at, inclusive.
    pub valid_for_seconds: u32,
}

impl Observation {
    fn fresh(&self, at: Option<DateTime<FixedOffset>>) -> bool {
        at.is_some_and(|at| {
            let age = at.signed_duration_since(self.observed_at);
            age >= chrono::Duration::zero()
                && age <= chrono::Duration::seconds(i64::from(self.valid_for_seconds))
        })
    }

    fn validate(&self) -> Result<()> {
        text(&self.source)?;
        if !(1..=3600).contains(&self.valid_for_seconds) {
            return Err(invalid("observation valid_for_seconds must be 1..=3600"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BusCrowding {
    Available,
    Crowded,
    /// No boarding capacity; this is stronger than having no seated places.
    BoardingUnavailable,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BusObservation {
    pub observation: Observation,
    pub crowding: BusCrowding,
    pub remaining_seats: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoadTraffic {
    Observed {
        observation: Observation,
        /// Incremental delay NOT already included in total_minutes, at most 86400 seconds.
        additional_seconds: u32,
    },
    Synthetic {
        /// Explicit hypothetical incremental delay, never an observed or predicted value.
        additional_seconds: u32,
    },
}

impl TravelOption {
    pub(super) fn validate_traffic(&self) -> Result<()> {
        match self.mode {
            Mode::Subway | Mode::Walk
                if self.bus_observation.is_some() || self.road_traffic.is_some() =>
            {
                return Err(invalid(
                    "bus observations and road traffic require bus or mixed mode",
                ));
            }
            Mode::Bus | Mode::Mixed | Mode::Subway | Mode::Walk => {}
        }
        if let Some(bus) = &self.bus_observation {
            bus.observation.validate()?;
            if bus.remaining_seats.is_some_and(|seats| seats > 200) {
                return Err(invalid("remaining_seats must be 0..=200"));
            }
            match bus.crowding {
                BusCrowding::BoardingUnavailable
                    if bus.remaining_seats.is_some_and(|seats| seats > 0) =>
                {
                    return Err(invalid(
                        "boarding_unavailable conflicts with positive remaining seats",
                    ));
                }
                BusCrowding::Available
                | BusCrowding::Crowded
                | BusCrowding::BoardingUnavailable => {}
            }
        }
        if let Some(traffic) = &self.road_traffic {
            let seconds = match traffic {
                RoadTraffic::Observed {
                    observation,
                    additional_seconds,
                } => {
                    observation.validate()?;
                    additional_seconds
                }
                RoadTraffic::Synthetic { additional_seconds } => additional_seconds,
            };
            if *seconds > 86400 {
                return Err(invalid("road additional_seconds must be 0..=86400"));
            }
        }
        Ok(())
    }

    pub(super) fn bus_at(&self, request: &TravelOptionsRequest) -> Result<Option<&BusObservation>> {
        let bus = self
            .bus_observation
            .as_ref()
            .filter(|bus| bus.observation.fresh(request.depart_at));
        if let Some(bus) = bus {
            match bus.crowding {
                BusCrowding::BoardingUnavailable => {
                    return Err(invalid(
                        "bus boarding unavailable; next-bus waiting is unknown",
                    ));
                }
                BusCrowding::Crowded if request.avoid_crowded_bus => {
                    return Err(invalid(
                        "crowded bus excluded by avoid_crowded_bus preference",
                    ));
                }
                BusCrowding::Available | BusCrowding::Crowded => {}
            }
        }
        match self.mode {
            Mode::Bus | Mode::Mixed
                if request.require_bus_seat
                    && bus
                        .and_then(|bus| bus.remaining_seats)
                        .is_none_or(|seats| seats == 0) =>
            {
                return Err(invalid(
                    "a bus seat is required but no fresh positive remaining-seat observation is available",
                ));
            }
            Mode::Bus | Mode::Mixed | Mode::Subway | Mode::Walk => {}
        }
        Ok(bus)
    }

    pub(super) fn road_at(&self, at: Option<DateTime<FixedOffset>>) -> Option<(&RoadTraffic, u32)> {
        let traffic = self.road_traffic.as_ref()?;
        match traffic {
            RoadTraffic::Observed {
                observation,
                additional_seconds,
            } => observation
                .fresh(at)
                .then_some((traffic, *additional_seconds)),
            RoadTraffic::Synthetic { additional_seconds } => Some((traffic, *additional_seconds)),
        }
    }

    pub(super) fn traffic_warnings(&self, at: Option<DateTime<FixedOffset>>) -> Vec<String> {
        let mut warnings = Vec::new();
        let observations = [
            self.bus_observation
                .as_ref()
                .map(|bus| ("bus", &bus.observation)),
            match &self.road_traffic {
                Some(RoadTraffic::Observed { observation, .. }) => Some(("road", observation)),
                Some(RoadTraffic::Synthetic { .. }) | None => None,
            },
        ];
        for (kind, observation) in observations.into_iter().flatten() {
            if !observation.fresh(at) {
                warnings.push(format!("{}: {kind} observation ignored: depart_at is absent, before observed_at, or past its validity window.", self.id));
            }
        }
        warnings
    }
}
