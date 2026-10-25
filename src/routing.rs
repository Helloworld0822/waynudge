use crate::{
    error::{Result, invalid},
    routines::{Mode, Place, Routine},
};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Route {
    pub id: String,
    pub mode: Mode,
    pub total_minutes: u32,
    pub walk_minutes: u32,
    pub transfers: u32,
    pub cost_won: u32,
    pub collected_at: DateTime<Utc>,
    pub realtime: bool,
    pub delay_minutes: u32,
    pub origin: Place,
    pub destination: Place,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RouteSet {
    pub source: String,
    pub routes: Vec<Route>,
    pub warnings: Vec<String>,
}
pub struct CompareInput {
    pub routine: Routine,
    pub routes: RouteSet,
    pub current_route_id: Option<String>,
    pub now: DateTime<Utc>,
    pub target: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Comparison {
    pub selected: Option<Route>,
    pub candidates: Vec<Route>,
    pub should_switch: bool,
    pub reason: String,
    pub warnings: Vec<String>,
    pub source: String,
}
pub fn compare(input: &CompareInput) -> Result<Comparison> {
    input.routine.validate()?;
    let mut warnings = input.routes.warnings.clone();
    let mut candidates = Vec::new();
    for route in &input.routes.routes {
        if route.total_minutes == 0
            || route.walk_minutes > route.total_minutes
            || route.total_minutes > 1440
            || route.delay_minutes > 1440
        {
            return Err(invalid("route duration out of range"));
        }
        let age = input
            .now
            .signed_duration_since(route.collected_at)
            .num_seconds();
        if !route.realtime {
            warnings.push("Real-time status unavailable; scheduled estimates only.".into());
        }
        if !(-30..=300).contains(&age) {
            warnings.push(
                "Route information is stale or has a future timestamp; refresh before departure."
                    .into(),
            );
        }
        let mut adjusted = route.clone();
        let walking = std::time::Duration::try_from_secs_f64(
            f64::from(route.walk_minutes) * input.routine.walking_factor * 60.0,
        )
        .map_err(|_| invalid("walking duration out of range"))?;
        adjusted.walk_minutes = u32::try_from(walking.as_secs().div_ceil(60))
            .map_err(|_| invalid("walking duration out of range"))?;
        adjusted.total_minutes = route.total_minutes - route.walk_minutes + adjusted.walk_minutes;
        adjusted.delay_minutes = if (route.realtime && (-30..=300).contains(&age))
            || (!route.realtime && input.routes.source == "mock")
        {
            route.delay_minutes
        } else {
            0
        };
        if adjusted.transfers <= input.routine.max_transfers
            && adjusted.walk_minutes <= input.routine.max_walk_minutes
            && input.routine.preferred_modes.contains(&adjusted.mode)
        {
            candidates.push(adjusted);
        }
    }
    candidates.sort_by(|a, b| {
        duration(a)
            .cmp(&duration(b))
            .then(a.transfers.cmp(&b.transfers))
            .then(a.walk_minutes.cmp(&b.walk_minutes))
            .then(a.cost_won.cmp(&b.cost_won))
            .then(a.id.cmp(&b.id))
    });
    let current = input
        .current_route_id
        .as_ref()
        .or(input.routine.usual_route_id.as_ref());
    let incumbent = current.and_then(|id| candidates.iter().find(|route| &route.id == id));
    let mut selected = candidates.first();
    let mut should_switch = false;
    let mut reason = "Fastest feasible estimate under your travel limits.";
    if let (Some(old), Some(best)) = (incumbent, selected) {
        let burden = 2_u32
            .saturating_add(
                best.transfers
                    .saturating_sub(old.transfers)
                    .saturating_mul(2),
            )
            .saturating_add(best.walk_minutes.saturating_sub(old.walk_minutes));
        let savings = duration(old).saturating_sub(duration(best));
        let available = input.target.signed_duration_since(input.now).num_minutes()
            - i64::from(input.routine.preparation_minutes)
            - i64::from(input.routine.buffer_minutes);
        let saves_arrival = i64::from(duration(old)) > available
            && i64::from(duration(best).saturating_add(burden)) <= available;
        should_switch = old.id != best.id
            && (savings >= burden.saturating_add(5) || (saves_arrival && savings >= 5));
        if should_switch {
            reason = if saves_arrival {
                "Alternative meaningfully improves the estimated arrival after switching burden."
            } else {
                "Estimated saving exceeds five minutes plus switching burden."
            };
        } else {
            selected = Some(old);
            reason = "Keep the current route; saving does not justify switching.";
        }
    } else if selected.is_some() && current.is_some() {
        should_switch = true;
        reason = "Current route is unavailable or violates your limits; choose a feasible route.";
    }
    if selected.is_none() {
        reason = "No route satisfies your travel limits.";
    }
    let selected = selected.cloned();
    warnings.sort();
    warnings.dedup();
    Ok(Comparison {
        selected,
        candidates,
        should_switch,
        reason: reason.into(),
        warnings,
        source: input.routes.source.clone(),
    })
}
fn duration(route: &Route) -> u32 {
    route.total_minutes.saturating_add(route.delay_minutes)
}
