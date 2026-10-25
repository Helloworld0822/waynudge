use super::*;
use crate::error::Result;
use serde_json::{Value, json};

fn fixture() -> Value {
    json!({
        "origin":{"name":"Synthetic origin","lat":37.5,"lon":127.0},
        "destination":{"name":"Synthetic destination","lat":37.6,"lon":127.1},
        "data_source":"mock", "congestion_percentages":[100],
        "depart_at":"2026-10-25T08:00:00+09:00",
        "options":[
            {"id":"bus","label":"Synthetic bus","mode":"bus","total_minutes":20,"walk_minutes":2,"cost_won":1500},
            {"id":"walk","label":"Synthetic walk","mode":"walk","total_minutes":25,"walk_minutes":25}
        ]
    })
}
fn observed() -> Value {
    json!({"source":"synthetic observation fixture","observed_at":"2026-10-25T07:59:00+09:00","valid_for_seconds":120})
}
fn compare(value: Value) -> Result<TravelOptionsReport> {
    compare_travel_options(serde_json::from_value(value)?)
}

#[test]
fn observed_road_delay_changes_ranking_without_rounding_or_double_counting() -> Result<()> {
    // Given
    let mut value = fixture();
    value["options"][0]["road_traffic"] =
        json!({"kind":"observed","observation":observed(),"additional_seconds":301});
    // When
    let report = compare(value)?;
    // Then
    assert_eq!(
        report.scenarios[0].selected_option_id.as_deref(),
        Some("walk")
    );
    assert_eq!(report.scenarios[0].candidates[1].total_seconds, 1501);
    assert_eq!(report.scenarios[0].candidates[1].road_delay_seconds, 301);
    Ok(())
}

#[test]
fn unavailable_boarding_excludes_bus_without_invented_next_bus_wait() -> Result<()> {
    // Given
    let mut value = fixture();
    value["options"][0]["bus_observation"] =
        json!({"observation":observed(),"crowding":"boarding_unavailable","remaining_seats":0});
    // When
    let report = compare(value)?;
    // Then
    assert_eq!(
        report.scenarios[0].selected_option_id.as_deref(),
        Some("walk")
    );
    assert_eq!(report.scenarios[0].candidates.len(), 1);
    Ok(())
}

#[test]
fn zero_seats_permits_standing_unless_seat_required() -> Result<()> {
    for requires_seat in [false, true] {
        // Given
        let mut value = fixture();
        value["require_bus_seat"] = json!(requires_seat);
        value["options"][0]["bus_observation"] =
            json!({"observation":observed(),"crowding":"crowded","remaining_seats":0});
        // When
        let report = compare(value)?;
        // Then
        assert_eq!(
            report.scenarios[0].selected_option_id.as_deref(),
            Some(if requires_seat { "walk" } else { "bus" })
        );
    }
    Ok(())
}

#[test]
fn crowd_avoidance_excludes_crowded_bus_even_with_a_seat() -> Result<()> {
    // Given
    let mut value = fixture();
    value["avoid_crowded_bus"] = json!(true);
    value["options"][0]["bus_observation"] =
        json!({"observation":observed(),"crowding":"crowded","remaining_seats":1});
    // When
    let report = compare(value)?;
    // Then
    assert_eq!(
        report.scenarios[0].selected_option_id.as_deref(),
        Some("walk")
    );
    Ok(())
}

#[test]
fn stale_future_and_undated_observations_do_not_affect_ranking() -> Result<()> {
    for depart in [
        json!("2026-10-25T08:01:00.001+09:00"),
        json!("2026-10-25T07:58:59+09:00"),
        Value::Null,
    ] {
        // Given
        let mut value = fixture();
        value["depart_at"] = depart;
        value["options"][0]["bus_observation"] =
            json!({"observation":observed(),"crowding":"boarding_unavailable","remaining_seats":0});
        value["options"][0]["road_traffic"] =
            json!({"kind":"observed","observation":observed(),"additional_seconds":900});
        // When
        let report = compare(value)?;
        // Then
        assert_eq!(
            report.scenarios[0].selected_option_id.as_deref(),
            Some("bus")
        );
        assert_eq!(report.scenarios[0].candidates[0].total_seconds, 1200);
        assert!(
            report.scenarios[0].candidates[0]
                .applied_bus_observation
                .is_none()
        );
        assert!(
            report.scenarios[0].candidates[0]
                .applied_road_traffic
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn invalid_traffic_and_nonbus_modes_are_rejected() {
    for (field, payload) in [
        (
            "road_traffic",
            json!({"kind":"synthetic","additional_seconds":-1}),
        ),
        (
            "road_traffic",
            json!({"kind":"synthetic","additional_seconds":86401}),
        ),
        (
            "road_traffic",
            json!({"kind":"observed","observation":{"source":"x","observed_at":"bad","valid_for_seconds":60},"additional_seconds":1}),
        ),
        (
            "bus_observation",
            json!({"observation":observed(),"crowding":"available","remaining_seats":201}),
        ),
        (
            "bus_observation",
            json!({"observation":observed(),"crowding":"boarding_unavailable","remaining_seats":1}),
        ),
    ] {
        let mut value = fixture();
        value["options"][0][field] = payload;
        assert!(compare(value).is_err());
    }
    for mode in ["subway", "walk"] {
        let mut value = fixture();
        value["options"][0]["mode"] = json!(mode);
        value["options"][0]["walk_minutes"] = json!(20);
        value["options"][0]["road_traffic"] = json!({"kind":"synthetic","additional_seconds":10});
        assert!(compare(value).is_err());
    }
}

#[test]
fn exact_fare_profile_is_used_and_missing_youth_child_never_fall_back_to_adult() -> Result<()> {
    for (profile, expected) in [("adult", 1700), ("youth", 900), ("child", 500)] {
        // Given
        let mut value = fixture();
        value["fare_profile"] = json!(profile);
        value["options"][0]["fares_won"] =
            json!({"source":"synthetic fares","adult":1700,"youth":900,"child":500});
        // When
        let report = compare(value)?;
        // Then
        assert_eq!(report.scenarios[0].candidates[0].cost_won, Some(expected));
        assert!(!report.scenarios[0].candidates[0].fare_verified);
    }
    for profile in ["youth", "child"] {
        let mut value = fixture();
        value["fare_profile"] = json!(profile);
        let report = compare(value)?;
        assert_eq!(report.scenarios[0].candidates[0].cost_won, None);
        assert_eq!(report.scenarios[0].candidates[0].fare_source, None);
    }
    Ok(())
}

#[test]
fn invalid_fare_quotes_are_rejected() {
    for fares in [
        json!({"source":"x","child":-1}),
        json!({"source":"x","youth":1000001}),
        json!({"source":"","adult":100}),
    ] {
        let mut value = fixture();
        value["options"][0]["fares_won"] = fares;
        assert!(compare(value).is_err());
    }
}
