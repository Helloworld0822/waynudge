use crate::error::{Result, invalid};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BusKind {
    Tago,
    Gyeonggi,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BusQuery {
    pub provider: BusKind,
    pub station_id: String,
    pub city_code: Option<u32>,
    pub route_id: Option<String>,
}
impl BusQuery {
    pub fn validate(&self) -> Result<()> {
        for id in std::iter::once(&self.station_id).chain(self.route_id.iter()) {
            if id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            {
                return Err(invalid("invalid bus station or route ID"));
            }
        }
        match self.provider {
            BusKind::Tago if !self.city_code.is_some_and(|code| code > 0) => {
                return Err(invalid("TAGO requires city_code"));
            }
            BusKind::Gyeonggi if !self.station_id.bytes().all(|c| c.is_ascii_digit()) => {
                return Err(invalid("Gyeonggi station_id must be numeric"));
            }
            BusKind::Tago | BusKind::Gyeonggi => {}
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BusCrowding {
    Relaxed,
    Normal,
    Crowded,
    VeryCrowded,
}
impl BusCrowding {
    pub(super) fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::Relaxed),
            2 => Some(Self::Normal),
            3 => Some(Self::Crowded),
            4 => Some(Self::VeryCrowded),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BusArrival {
    pub route_id: String,
    pub route_number: Option<String>,
    pub arrival_seconds: u32,
    pub stops_away: Option<u32>,
    pub remaining_seats: Option<u32>,
    pub crowding_code: Option<String>,
    pub crowding: Option<BusCrowding>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BusArrivals {
    pub source: String,
    pub station_id: String,
    pub collected_at: DateTime<Utc>,
    pub arrivals: Vec<BusArrival>,
    pub warnings: Vec<String>,
}
