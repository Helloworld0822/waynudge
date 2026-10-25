use super::*;
use crate::routines::Mode;

fn input() -> Result<TravelOptionsRequest> {
    Ok(serde_json::from_value(serde_json::json!({
        "origin":{"name":"Current station","lat":37.5,"lon":127.0},
        "destination":{"name":"Work","lat":37.6,"lon":127.1},
        "data_source":"user_provided_map", "congestion_percentages":[100.0],
        "walking_speed_kmh":6.0,
        "options":[{
            "id":"subway", "label":"Subway", "mode":"subway", "total_minutes":40,
            "walk_minutes":10, "walk_distance_meters":600.0, "station_walk_minutes":5,
            "station_walk_distance_meters":300.0, "remaining_train_stops":10
        }]
    }))?)
}

fn first(report: &TravelOptionsReport) -> Result<&TravelOptionsScenario> {
    report
        .scenarios
        .first()
        .ok_or_else(|| crate::error::invalid("missing test scenario"))
}

#[test]
fn measured_walking_preserves_vehicle_time_and_reuses_crowd_policy() -> Result<()> {
    let mut input = input()?;
    input.congestion_percentages = vec![200.0];
    let report = compare_travel_options(input)?;
    let candidate = &first(&report)?.candidates[0];
    assert_eq!(candidate.total_seconds, 2305);
    assert_eq!(candidate.walk_seconds, 405);
    assert_eq!(candidate.walking_buffer_seconds, 45);
    assert_eq!(candidate.train_dwell_buffer_seconds, 100);
    assert!(!report.realtime && !report.validated_prediction);
    Ok(())
}

#[test]
fn unknown_station_distance_keeps_whole_walking_baseline() -> Result<()> {
    let mut input = input()?;
    input.options[0].station_walk_distance_meters = None;
    let report = compare_travel_options(input)?;
    let scenario = first(&report)?;
    assert_eq!(scenario.candidates[0].total_seconds, 2400);
    assert_eq!(scenario.candidates[0].walk_seconds, 600);
    assert_eq!(scenario.warnings.len(), 1);
    Ok(())
}

#[test]
fn known_station_distance_corrects_only_that_part_when_total_distance_is_unknown() -> Result<()> {
    let mut input = input()?;
    input.options[0].walk_distance_meters = None;
    let report = compare_travel_options(input)?;
    let scenario = first(&report)?;
    assert_eq!(scenario.candidates[0].total_seconds, 2280);
    assert_eq!(scenario.candidates[0].walk_seconds, 480);
    assert_eq!(scenario.warnings.len(), 1);
    Ok(())
}

#[test]
fn bus_and_whole_walk_receive_no_train_dwell_or_station_buffer() -> Result<()> {
    let mut input = input()?;
    input.congestion_percentages = vec![400.0];
    let bus = &mut input.options[0];
    bus.mode = Mode::Bus;
    bus.station_walk_minutes = 0;
    bus.station_walk_distance_meters = None;
    bus.remaining_train_stops = 0;
    let mut walk = bus.clone();
    walk.id = "walk".into();
    walk.mode = Mode::Walk;
    walk.total_minutes = 10;
    input.options.push(walk);
    let report = compare_travel_options(input)?;
    let candidates = &first(&report)?.candidates;
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].total_seconds, 360);
    assert_eq!(candidates[1].total_seconds, 2160);
    for candidate in candidates {
        assert_eq!(candidate.walking_buffer_seconds, 0);
        assert_eq!(candidate.train_dwell_buffer_seconds, 0);
    }
    Ok(())
}

#[test]
fn excessive_corrected_duration_excludes_only_that_option() -> Result<()> {
    let mut input = input()?;
    let mut long = input.options[0].clone();
    long.id = "long".into();
    long.walk_distance_meters = Some(10_000_000.0);
    input.options.push(long);
    let report = compare_travel_options(input)?;
    let scenario = first(&report)?;
    assert_eq!(scenario.candidates.len(), 1);
    assert_eq!(scenario.selected_option_id.as_deref(), Some("subway"));
    assert_eq!(scenario.warnings.len(), 1);
    Ok(())
}

#[test]
fn switching_can_choose_second_fastest_when_fastest_has_excessive_burden() -> Result<()> {
    let mut input = input()?;
    input.walking_speed_kmh = None;
    input.current_option_id = Some("subway".into());
    let current = &mut input.options[0];
    current.total_minutes = 45;
    current.walk_minutes = 5;
    let mut fastest = current.clone();
    fastest.id = "fastest".into();
    fastest.total_minutes = 35;
    fastest.walk_minutes = 15;
    fastest.transfers = 2;
    let mut alternative = current.clone();
    alternative.id = "alternative".into();
    alternative.total_minutes = 37;
    input.options.extend([fastest, alternative]);
    let report = compare_travel_options(input)?;
    let scenario = first(&report)?;
    assert_eq!(scenario.candidates[0].id, "fastest");
    assert_eq!(scenario.selected_option_id.as_deref(), Some("alternative"));
    assert!(scenario.should_switch);
    Ok(())
}

#[test]
fn limits_apply_to_buffered_walking_and_can_remove_incumbent() -> Result<()> {
    let mut input = input()?;
    input.congestion_percentages = vec![400.0];
    input.current_option_id = Some("subway".into());
    input.max_walk_minutes = Some(6);
    let mut bus = input.options[0].clone();
    bus.id = "bus".into();
    bus.mode = Mode::Bus;
    bus.remaining_train_stops = 0;
    bus.station_walk_minutes = 0;
    bus.station_walk_distance_meters = None;
    input.options.push(bus);
    let report = compare_travel_options(input)?;
    let scenario = first(&report)?;
    assert_eq!(scenario.selected_option_id.as_deref(), Some("bus"));
    assert!(scenario.should_switch);
    assert_eq!(scenario.candidates.len(), 1);
    Ok(())
}

#[test]
fn duplicate_ids_and_impossible_bus_stops_fail_validation() -> Result<()> {
    let mut duplicates = input()?;
    duplicates.options.push(duplicates.options[0].clone());
    assert!(compare_travel_options(duplicates).is_err());
    let mut bus = input()?;
    bus.options[0].mode = Mode::Bus;
    assert!(compare_travel_options(bus).is_err());
    Ok(())
}
