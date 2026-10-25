mod evaluation;
mod model;
mod validation;

pub use model::{
    RouteDataSource, TravelOption, TravelOptionCandidate, TravelOptionsReport,
    TravelOptionsRequest, TravelOptionsScenario,
};

use crate::error::{Error, Result};

pub fn compare_travel_options(input: TravelOptionsRequest) -> Result<TravelOptionsReport> {
    input.validate()?;
    let mut scenarios = Vec::with_capacity(input.congestion_percentages.len());
    for percent in &input.congestion_percentages {
        let mut candidates = Vec::with_capacity(input.options.len());
        let mut warnings = Vec::new();
        for option in &input.options {
            let candidate = match option.evaluate(&input, *percent) {
                Ok(candidate) => candidate,
                Err(Error::Invalid(reason)) => {
                    warnings.push(format!("{} excluded: {}", option.id, reason));
                    continue;
                }
                Err(error) => return Err(error),
            };
            if let Some(warning) = option.calibration_warning(input.walking_speed_kmh) {
                warnings.push(warning);
            }
            if (!input.preferred_modes.is_empty() && !input.preferred_modes.contains(&option.mode))
                || input
                    .max_transfers
                    .is_some_and(|max| candidate.transfers > max)
                || input
                    .max_walk_minutes
                    .is_some_and(|max| candidate.walk_seconds > max * 60)
            {
                warnings.push(format!(
                    "{} excluded by mode, transfer or walking limits in this scenario.",
                    option.id
                ));
                continue;
            }
            candidates.push(candidate);
        }
        candidates.sort_by(|a, b| {
            a.total_seconds
                .cmp(&b.total_seconds)
                .then(a.transfers.cmp(&b.transfers))
                .then(a.walk_seconds.cmp(&b.walk_seconds))
                .then(a.cost_won.is_none().cmp(&b.cost_won.is_none()))
                .then(a.cost_won.cmp(&b.cost_won))
                .then(a.id.cmp(&b.id))
        });
        let (selected, reason) = select(&candidates, input.current_option_id.as_deref());
        let selected_option_id = selected.map(|option| option.id.clone());
        let should_switch = input
            .current_option_id
            .as_ref()
            .zip(selected_option_id.as_ref())
            .is_some_and(|(current, selected)| current != selected);
        scenarios.push(TravelOptionsScenario {
            congestion_percent: *percent,
            candidates,
            selected_option_id,
            should_switch,
            reason: reason.into(),
            warnings,
        });
    }
    let source_warning = match input.data_source {
        RouteDataSource::Mock => {
            "Mock paths are illustrative inputs and are not verified available routes."
        }
        RouteDataSource::UserProvidedMap => {
            "Paths, durations and distances are caller-supplied map data; no route API fetch or GPS tracking is performed."
        }
    };
    Ok(TravelOptionsReport {
        data_source: input.data_source,
        source: "simulation".into(),
        realtime: false,
        validated_prediction: false,
        origin: input.origin,
        destination: input.destination,
        scenarios,
        warnings: vec![
            source_warning.into(),
            "Congestion percentages use an unvalidated planning policy, not live observations or arrival guarantees. Train occupancy does not measure station pedestrian density.".into(),
            "Bus road traffic and missed-vehicle waiting are not modeled; supplied vehicle time is preserved. Known-distance walking is corrected for speed and rounded up to whole minutes.".into(),
        ],
    })
}

fn select<'a>(
    candidates: &'a [TravelOptionCandidate],
    current_id: Option<&str>,
) -> (Option<&'a TravelOptionCandidate>, &'static str) {
    let Some(best) = candidates.first() else {
        return (
            None,
            "No supplied option satisfies the scenario preferences.",
        );
    };
    let Some(current_id) = current_id else {
        return (Some(best), "Selected the fastest feasible supplied option.");
    };
    let Some(current) = candidates.iter().find(|option| option.id == current_id) else {
        return (
            Some(best),
            "Current option is unavailable or infeasible; selected the fastest feasible supplied option.",
        );
    };
    let alternative = candidates.iter().find(|option| {
        let extra_transfers = option.transfers.saturating_sub(current.transfers);
        let extra_walk = option.walk_seconds.saturating_sub(current.walk_seconds);
        let threshold = 5 * 60 + 2 * 60 + extra_transfers * 2 * 60 + extra_walk;
        current.total_seconds.saturating_sub(option.total_seconds) >= threshold
    });
    match alternative {
        Some(option) => (
            Some(option),
            "Alternative saves at least five minutes after switching, additional-transfer and extra-walking burdens.",
        ),
        None => (
            Some(current),
            "Retained the current option because alternatives do not clear the switching threshold.",
        ),
    }
}

#[cfg(test)]
mod tests;
