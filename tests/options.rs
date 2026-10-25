use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::collections::HashMap;
use tower::ServiceExt;
use waynudge::{
    api::{self, Auth},
    options::{TravelOptionsRequest, compare_travel_options},
    providers::Provider,
    service::Service,
    storage::Store,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn synthetic_bus_traffic_fixture_keeps_youth_quote_and_adds_only_road_delay() -> TestResult {
    let request = serde_json::from_str(include_str!("../examples/bus-traffic.json"))?;
    let report = compare_travel_options(request)?;
    let bus = report.scenarios[0]
        .candidates
        .iter()
        .find(|candidate| candidate.id == "mock-bus")
        .ok_or("missing bus")?;
    assert_eq!(bus.total_seconds, 2580);
    assert_eq!(bus.road_delay_seconds, 600);
    assert_eq!(bus.cost_won, Some(850));
    Ok(())
}

fn input() -> Result<TravelOptionsRequest, serde_json::Error> {
    serde_json::from_value(json!({
        "origin":{"name":"Reported current station","lat":37.5,"lon":127.0},
        "destination":{"name":"Test destination","lat":37.6,"lon":127.1},
        "data_source":"mock","congestion_percentages":[80,400],"walking_speed_kmh":5.0,"current_option_id":"subway",
        "options":[
            {"id":"subway","label":"Stay on subway","mode":"subway","total_minutes":37,"walk_minutes":6,"walk_distance_meters":500,"station_walk_minutes":6,"station_walk_distance_meters":500,"remaining_train_stops":10,"transfers":1,"cost_won":1600},
            {"id":"bus","label":"Switch to bus here","mode":"bus","total_minutes":39,"walk_minutes":4,"walk_distance_meters":300,"transfers":0,"cost_won":1500},
            {"id":"mixed","label":"Bus and subway alternative","mode":"mixed","total_minutes":35,"walk_minutes":8,"walk_distance_meters":600,"station_walk_minutes":3,"station_walk_distance_meters":250,"remaining_train_stops":4,"transfers":2,"cost_won":1800},
            {"id":"walk","label":"Walk all the way","mode":"walk","total_minutes":45,"walk_minutes":45,"walk_distance_meters":3750,"transfers":0,"cost_won":0}
        ]
    }))
}
#[test]
fn compares_bus_subway_mixed_and_walk_with_stable_switch_threshold() -> TestResult {
    let report = serde_json::to_value(compare_travel_options(input()?)?)?;
    assert_eq!(report["source"], "simulation");
    assert_eq!(report["realtime"], false);
    assert_eq!(
        report["scenarios"][0]["candidates"]
            .as_array()
            .ok_or("missing candidates")?
            .len(),
        4
    );
    assert_eq!(report["scenarios"][0]["selected_option_id"], "subway");
    assert_eq!(report["scenarios"][0]["should_switch"], false);
    let high = &report["scenarios"][1];
    assert_eq!(high["selected_option_id"], "bus");
    assert_eq!(high["should_switch"], true);
    assert_eq!(high["candidates"][0]["id"], "bus");
    assert_eq!(high["candidates"][0]["train_dwell_buffer_seconds"], 0);
    assert_eq!(report["origin"]["name"], "Reported current station");
    Ok(())
}
#[test]
fn walking_profile_adjusts_only_walking_part_of_bus_option() -> TestResult {
    let mut request = input()?;
    request.walking_speed_kmh = Some(6.0);
    request.congestion_percentages = vec![100.0];
    let report = serde_json::to_value(compare_travel_options(request)?)?;
    let bus = report["scenarios"][0]["candidates"]
        .as_array()
        .ok_or("missing candidates")?
        .iter()
        .find(|r| r["id"] == "bus")
        .ok_or("missing bus")?;
    assert_eq!(bus["walk_seconds"], 180);
    assert_eq!(bus["total_seconds"], 2280);
    assert_eq!(
        bus["total_seconds"].as_u64().ok_or("missing duration")?
            - bus["walk_seconds"].as_u64().ok_or("missing walk")?,
        35 * 60
    );
    Ok(())
}
#[tokio::test]
async fn public_mcp_can_compare_multiple_paths_without_saved_routine() -> TestResult {
    let service = Service {
        store: Store::open("sqlite::memory:").await?,
        provider: Provider::mock(),
    };
    let auth = Auth::new(
        HashMap::from([(
            "owner".into(),
            "multi-option-owner-token-at-least-32-characters".into(),
        )]),
        None,
    )?
    .with_public_mcp(true);
    let app = api::router(service.clone(), auth);
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"compare_travel_options","arguments":serde_json::to_value(input()?)?}});
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await?)?;
    assert_eq!(
        body["result"]["structuredContent"]["scenarios"][1]["selected_option_id"],
        "bus"
    );
    assert!(service.store.list("owner").await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn public_mcp_applies_incremental_road_delay_and_child_fare() -> TestResult {
    // Given: a synthetic delay not included in the bus baseline, plus explicit child fare.
    let service = Service {
        store: Store::open("sqlite::memory:").await?,
        provider: Provider::mock(),
    };
    let auth = Auth::new(
        HashMap::from([(
            "owner".into(),
            "traffic-owner-token-at-least-32-characters".into(),
        )]),
        None,
    )?
    .with_public_mcp(true);
    let mut arguments = serde_json::to_value(input()?)?;
    arguments["fare_profile"] = json!("child");
    arguments["depart_at"] = json!("2026-10-25T08:00:00+09:00");
    arguments["target_arrival"] = json!("2026-10-25T08:45:00+09:00");
    arguments["options"][1]["road_traffic"] = json!({"kind":"synthetic","additional_seconds":900});
    arguments["options"][1]["fares_won"] =
        json!({"source":"synthetic fixture","adult":1500,"youth":800,"child":400});
    let body = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"compare_travel_options","arguments":arguments}});
    // When: the real MCP router handles the comparison.
    let response = api::router(service, auth)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await?)?;
    let report = &body["result"]["structuredContent"];
    // Then: road traffic reverses the previous high-congestion selection and updates arrival exactly once.
    assert_eq!(report["scenarios"][1]["selected_option_id"], "subway");
    let bus = report["scenarios"][1]["candidates"]
        .as_array()
        .ok_or("missing candidates")?
        .iter()
        .find(|candidate| candidate["id"] == "bus")
        .ok_or("missing bus")?;
    assert_eq!(bus["road_delay_seconds"], 900);
    assert_eq!(bus["total_seconds"], 3240);
    assert_eq!(bus["cost_won"], 400);
    assert_eq!(bus["fare_profile"], "child");
    assert_eq!(bus["fare_verified"], false);
    assert_eq!(bus["applied_road_traffic"]["kind"], "synthetic");
    assert_eq!(bus["meets_target"], false);
    assert_eq!(bus["arrival_at"], "2026-10-25T08:54:00+09:00");
    Ok(())
}
