use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::routines::{Mode, Place};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RouteDataSource {
    Mock,
    UserProvidedMap,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TravelOptionsRequest {
    pub origin: Place,
    pub destination: Place,
    pub data_source: RouteDataSource,
    pub options: Vec<TravelOption>,
    pub congestion_percentages: Vec<f64>,
    pub walking_speed_kmh: Option<f64>,
    pub depart_at: Option<DateTime<FixedOffset>>,
    pub target_arrival: Option<DateTime<FixedOffset>>,
    pub current_option_id: Option<String>,
    #[serde(default)]
    pub preferred_modes: Vec<Mode>,
    pub max_walk_minutes: Option<u32>,
    pub max_transfers: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TravelOption {
    pub id: String,
    pub label: String,
    pub mode: Mode,
    pub total_minutes: u32,
    pub walk_minutes: u32,
    pub walk_distance_meters: Option<f64>,
    #[serde(default)]
    pub station_walk_minutes: u32,
    pub station_walk_distance_meters: Option<f64>,
    #[serde(default)]
    pub remaining_train_stops: u32,
    #[serde(default)]
    pub transfers: u32,
    pub cost_won: Option<u32>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TravelOptionCandidate {
    pub id: String,
    pub label: String,
    pub mode: Mode,
    pub total_seconds: u32,
    pub walk_seconds: u32,
    pub walking_buffer_seconds: u32,
    pub train_dwell_buffer_seconds: u32,
    pub transfers: u32,
    pub cost_won: Option<u32>,
    pub arrival_at: Option<String>,
    pub meets_target: Option<bool>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TravelOptionsScenario {
    pub congestion_percent: f64,
    pub candidates: Vec<TravelOptionCandidate>,
    pub selected_option_id: Option<String>,
    pub should_switch: bool,
    pub reason: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TravelOptionsReport {
    pub data_source: RouteDataSource,
    pub source: String,
    pub realtime: bool,
    pub validated_prediction: bool,
    pub origin: Place,
    pub destination: Place,
    pub scenarios: Vec<TravelOptionsScenario>,
    pub warnings: Vec<String>,
}
