use crate::error::{Result, invalid};
use chrono::{NaiveDate, NaiveTime};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Place {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}
impl Place {
    pub fn validate(&self) -> Result<()> {
        text(&self.name)?;
        if !self.lat.is_finite()
            || !self.lon.is_finite()
            || !(-90.0..=90.0).contains(&self.lat)
            || !(-180.0..=180.0).contains(&self.lon)
        {
            return Err(invalid("coordinates out of range"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Subway,
    Bus,
    Mixed,
    Walk,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub origin: Place,
    pub destination: Place,
    /// Local arrival time, HH:MM, Asia/Seoul.
    pub arrival_time: String,
    /// Monday=1 through Sunday=7.
    pub weekdays: Vec<u32>,
    pub preferred_modes: Vec<Mode>,
    pub max_transfers: u32,
    pub max_walk_minutes: u32,
    pub buffer_minutes: u32,
    pub preparation_minutes: u32,
    pub min_alert_interval_minutes: u32,
    /// Walking duration multiplier; 1.2 means 20% slower than provider baseline.
    pub walking_factor: f64,
    pub usual_route_id: Option<String>,
}
impl Routine {
    pub fn validate(&self) -> Result<()> {
        text(&self.id)?;
        text(&self.name)?;
        self.origin.validate()?;
        self.destination.validate()?;
        time(&self.arrival_time)?;
        if self.weekdays.is_empty()
            || self.weekdays.len() > 7
            || self.weekdays.iter().any(|d| !(1..=7).contains(d))
            || self.preferred_modes.is_empty()
            || self.preferred_modes.len() > 4
        {
            return Err(invalid("weekdays 1..7 and preferred_modes required"));
        }
        if self.max_transfers > 10
            || self.max_walk_minutes > 180
            || self.buffer_minutes > 180
            || self.preparation_minutes > 180
            || !(1..=1440).contains(&self.min_alert_interval_minutes)
            || !self.walking_factor.is_finite()
            || !(0.5..=3.0).contains(&self.walking_factor)
        {
            return Err(invalid("routine limits out of range"));
        }
        if let Some(id) = &self.usual_route_id {
            text(id)?;
        }
        Ok(())
    }
}
pub fn text(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 200 || value.chars().any(char::is_control) {
        return Err(invalid(
            "text must be 1..200 bytes without control characters",
        ));
    }
    Ok(())
}
pub fn time(value: &str) -> Result<NaiveTime> {
    if value.len() != 5 {
        return Err(invalid("time must be HH:MM"));
    }
    NaiveTime::parse_from_str(value, "%H:%M").map_err(|_| invalid("time must be HH:MM"))
}
pub fn date(value: &str) -> Result<NaiveDate> {
    if value.len() != 10 {
        return Err(invalid("date must be YYYY-MM-DD"));
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| invalid("date must be YYYY-MM-DD"))
}
