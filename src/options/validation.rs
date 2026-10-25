use std::collections::HashSet;

use crate::{
    error::{Result, invalid},
    routines::{Mode, text},
};

use super::{TravelOption, TravelOptionsRequest};

impl TravelOptionsRequest {
    pub(super) fn validate(&self) -> Result<()> {
        self.origin.validate()?;
        self.destination.validate()?;
        if !(1..=20).contains(&self.options.len())
            || !(1..=10).contains(&self.congestion_percentages.len())
            || self
                .congestion_percentages
                .iter()
                .any(|p| !p.is_finite() || !(0.0..=400.0).contains(p))
            || self
                .walking_speed_kmh
                .is_some_and(|speed| !speed.is_finite() || !(0.5..=12.0).contains(&speed))
            || self.preferred_modes.len() > 4
            || self.max_walk_minutes.is_some_and(|minutes| minutes > 1440)
            || self.max_transfers.is_some_and(|transfers| transfers > 10)
        {
            return Err(invalid(
                "invalid travel options, congestion values, walking speed or preferences",
            ));
        }
        if let Some(id) = &self.current_option_id {
            text(id)?;
        }
        let mut ids = HashSet::new();
        for option in &self.options {
            option.validate()?;
            if !ids.insert(&option.id) {
                return Err(invalid("travel option IDs must be unique"));
            }
        }
        Ok(())
    }
}

impl TravelOption {
    fn validate(&self) -> Result<()> {
        self.validate_traffic()?;
        self.validate_fares()?;
        text(&self.id)?;
        text(&self.label)?;
        if !(1..=1440).contains(&self.total_minutes)
            || self.walk_minutes > self.total_minutes
            || self.station_walk_minutes > self.walk_minutes
            || self.transfers > 10
            || self.remaining_train_stops > 200
            || [self.walk_distance_meters, self.station_walk_distance_meters]
                .into_iter()
                .flatten()
                .any(|distance| !distance.is_finite() || distance < 0.0)
        {
            return Err(invalid(
                "travel option durations, distances, transfers or stops out of range",
            ));
        }
        match self.mode {
            Mode::Bus if self.remaining_train_stops > 0 => {
                return Err(invalid("bus options cannot contain train stops"));
            }
            Mode::Walk
                if self.remaining_train_stops > 0
                    || self.station_walk_minutes > 0
                    || self
                        .station_walk_distance_meters
                        .is_some_and(|distance| distance > 0.0)
                    || self.walk_minutes != self.total_minutes =>
            {
                return Err(invalid(
                    "whole-trip walking requires all duration to be walking and no station walking or train stops",
                ));
            }
            Mode::Subway | Mode::Bus | Mode::Mixed | Mode::Walk => {}
        }
        Ok(())
    }
}
