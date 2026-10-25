use super::*;
use serde_json::json;
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
fn input() -> Result<SimulationRequest> {
    Ok(serde_json::from_value(json!({
        "congestion_percentages":[80,150,200],
        "context":{"base_trip_minutes":40,"station_walk_minutes":6,"remaining_train_stops":10,"target_arrival":"2026-10-26T09:00:00+09:00"},
        "depart_at":"2026-10-26T08:19:00+09:00",
        "boarding":{"walk_minutes":6,"train_departure_after_seconds":390},
        "walking_alternative_minutes":41
    }))?)
}
#[test]
fn congestion_changes_boarding_deadline_and_walking_comparison_without_probabilities() -> TestResult
{
    let report = simulate(input()?)?;
    assert_eq!(report.source, "simulation");
    assert!(!report.realtime);
    assert!(!report.validated_prediction);
    let low = &report.scenarios[0];
    let high = &report.scenarios[1];
    assert_eq!(low.catches_train_in_scenario, Some(true));
    assert_eq!(high.catches_train_in_scenario, Some(false));
    assert_eq!(low.meets_target_in_scenario, Some(true));
    assert_eq!(high.meets_target_in_scenario, None);
    assert_eq!(high.arrival_at_with_buffer, None);
    let walk = high
        .walking_alternative
        .as_ref()
        .ok_or("missing walk alternative")?;
    assert_eq!(walk.faster_than_transit_in_scenario, None);
    assert_eq!(walk.meets_target, Some(true));
    assert_eq!(high.calculation.total_buffer_seconds, 95);
    assert!(
        report.scenarios[2].calculation.trip_seconds_with_buffer
            > high.calculation.trip_seconds_with_buffer
    );
    Ok(())
}
#[test]
fn known_boarding_allows_deadline_and_walking_comparison() -> TestResult {
    let mut request = input()?;
    request
        .boarding
        .as_mut()
        .ok_or("missing boarding")?
        .train_departure_after_seconds = 600;
    let result = simulate(request)?;
    let high = &result.scenarios[1];
    assert_eq!(high.meets_target_in_scenario, Some(false));
    assert_eq!(
        high.arrival_at_with_buffer.as_deref(),
        Some("2026-10-26T09:00:35+09:00")
    );
    assert_eq!(
        high.walking_alternative
            .as_ref()
            .ok_or("missing walk alternative")?
            .faster_than_transit_in_scenario,
        Some(true)
    );
    Ok(())
}
#[test]
fn missing_times_and_walk_route_remain_unknown_and_inputs_are_bounded() -> TestResult {
    let mut request = input()?;
    request.depart_at = None;
    request.boarding = None;
    request.walking_alternative_minutes = None;
    let report = simulate(request)?;
    assert!(report.scenarios[0].meets_target_in_scenario.is_none());
    assert!(report.scenarios[0].walking_alternative.is_none());
    assert!(report.scenarios[0].catches_train_in_scenario.is_none());
    let mut request = input()?;
    request.congestion_percentages = vec![150.0; 11];
    assert!(simulate(request).is_err());
    let mut request = input()?;
    request.congestion_percentages = vec![-1.0];
    assert!(simulate(request).is_err());
    let mut request = input()?;
    request.congestion_percentages = vec![401.0];
    assert!(simulate(request).is_err());
    let mut request = input()?;
    request
        .boarding
        .as_mut()
        .ok_or("missing boarding")?
        .walk_minutes = 7;
    assert!(simulate(request).is_err());
    Ok(())
}
#[tokio::test]
async fn anonymous_mcp_simulates_without_api_keys_or_routine_writes() -> TestResult {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let service = crate::service::Service {
        provider: crate::providers::Provider::mock(),
        store: crate::storage::Store::open("sqlite::memory:").await?,
    };
    let auth = crate::api::Auth::new(
        std::collections::HashMap::from([(
            "owner".into(),
            "owner-token-more-than-thirty-two-characters".into(),
        )]),
        None,
    )?
    .with_public_mcp(true);
    let app = crate::api::router(service.clone(), auth);
    let arguments = json!({"congestion_percentages":[150],"context":{"base_trip_minutes":40,"station_walk_minutes":6,"remaining_train_stops":10}});
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"simulate_congestion","arguments":arguments}});
    let response = app
        .clone()
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
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await?)?;
    assert_eq!(body["result"]["structuredContent"]["source"], "simulation");
    assert_eq!(
        body["result"]["structuredContent"]["scenarios"][0]["calculation"]["total_buffer_seconds"],
        95
    );
    assert!(service.store.list("owner").await?.is_empty());
    assert_eq!(
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/simulate/congestion")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&arguments)?))?
        )
        .await?
        .status(),
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}
