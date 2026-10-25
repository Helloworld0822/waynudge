use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tower::ServiceExt;
use waynudge::{
    api::{self, Auth},
    providers::Provider,
    routines::Routine,
    service::{BriefRequest, Service},
    storage::Store,
    walking::WalkingObservation,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const TOKEN: &str = "walking-test-owner-token-more-than-32-characters";
fn routine() -> Result<Routine, serde_json::Error> {
    #[derive(Deserialize)]
    struct Wrapped {
        routine: Routine,
    }
    Ok(serde_json::from_str::<Wrapped>(include_str!("../examples/routine.json"))?.routine)
}
fn observation() -> Result<WalkingObservation, serde_json::Error> {
    serde_json::from_value(
        json!({"routine_id":"work","observation_id":"walk-test-1","origin":{"name":"Test origin","lat":37.5,"lon":127.0},"destination":{"name":"Test station","lat":37.5005,"lon":127.0},"distance_meters":1000,"elapsed_minutes":12,"distance_source":"user_measured","confirmed":false}),
    )
}
async fn setup() -> Result<Service, Box<dyn std::error::Error>> {
    let service = Service {
        store: Store::open("sqlite::memory:").await?,
        provider: Provider::mock(),
    };
    let mut routine = routine()?;
    routine.id = "work".into();
    service.store.upsert("alice", &routine).await?;
    Ok(service)
}
async fn mcp(
    app: Router,
    arguments: Value,
    token: Option<&str>,
) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
    let mut request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", "localhost")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"record_walking_observation","arguments":arguments}});
    let response = app
        .oneshot(request.body(Body::from(serde_json::to_vec(&body)?))?)
        .await?;
    let status = response.status();
    let body = serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await?)?;
    Ok((status, body))
}
#[tokio::test]
async fn preview_is_public_but_saving_requires_owner_auth_and_confirmation() -> TestResult {
    let service = setup().await?;
    let auth =
        Auth::new(HashMap::from([("alice".into(), TOKEN.into())]), None)?.with_public_mcp(true);
    let app = api::router(service.clone(), auth);
    let args = serde_json::to_value(observation()?)?;
    let (status, body) = mcp(app.clone(), args.clone(), None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["structuredContent"]["saved"], false);
    assert_eq!(
        body["result"]["structuredContent"]["measured_speed_kmh"],
        5.0
    );
    assert!(body["result"]["structuredContent"]["profile_speed_kmh"].is_null());
    assert_eq!(
        service.store.get("alice", "work").await?.walking_speed_kmh,
        None
    );
    let mut save = args;
    save["confirmed"] = json!(true);
    let (_, denied) = mcp(app.clone(), save.clone(), None).await?;
    assert!(denied.get("error").is_some() || denied["result"]["isError"] == true);
    let (_, saved) = mcp(app, save, Some(TOKEN)).await?;
    assert_eq!(saved["result"]["structuredContent"]["saved"], true);
    assert_eq!(
        saved["result"]["structuredContent"]["profile_speed_kmh"],
        5.0
    );
    Ok(())
}
#[tokio::test]
async fn measured_speed_changes_walk_transfer_and_departure_without_scaling_train_time()
-> TestResult {
    let service = setup().await?;
    let before = service
        .brief(
            "alice",
            BriefRequest {
                routine_id: "work".into(),
                date: "2026-10-26".into(),
                current_route_id: None,
            },
        )
        .await?;
    let before_route = before.comparison.selected.ok_or("no baseline route")?;
    let mut input = observation()?;
    input.confirmed = true;
    service.record_walking_observation("alice", input).await?;
    let after = service
        .brief(
            "alice",
            BriefRequest {
                routine_id: "work".into(),
                date: "2026-10-26".into(),
                current_route_id: None,
            },
        )
        .await?;
    let after_route = after.comparison.selected.ok_or("no calibrated route")?;
    assert_eq!(before_route.id, "mock-mixed");
    assert_eq!(after_route.id, "mock-mixed");
    assert_eq!(before_route.walk_minutes, 10);
    assert_eq!(after_route.walk_minutes, 8);
    assert_eq!(before_route.total_minutes, 35);
    assert_eq!(after_route.total_minutes, 33);
    assert_eq!(
        before_route.total_minutes - before_route.walk_minutes,
        after_route.total_minutes - after_route.walk_minutes
    );
    let old_leave = chrono::DateTime::parse_from_rfc3339(
        before.leave_at.as_deref().ok_or("missing baseline leave")?,
    )?;
    let new_leave = chrono::DateTime::parse_from_rfc3339(
        after
            .leave_at
            .as_deref()
            .ok_or("missing calibrated leave")?,
    )?;
    assert_eq!(
        new_leave.signed_duration_since(old_leave).num_seconds(),
        120
    );
    let before_board = before.boarding.ok_or("missing baseline boarding")?;
    let after_board = after.boarding.ok_or("missing calibrated boarding")?;
    let old_board = chrono::DateTime::parse_from_rfc3339(&before_board.planned_board_at)?;
    let new_board = chrono::DateTime::parse_from_rfc3339(&after_board.planned_board_at)?;
    assert_eq!(old_board, new_board);
    let profile = service.store.get("alice", "work").await?;
    assert!((profile.walking_duration_seconds(Some(500.0), 10)? - 360.0).abs() < 0.001);
    Ok(())
}
#[tokio::test]
async fn measurements_are_idempotent_weighted_and_isolated_between_users() -> TestResult {
    let service = setup().await?;
    let mut input = observation()?;
    input.confirmed = true;
    let first = service.record_walking_observation("alice", input).await?;
    assert_eq!(first.sample_count, Some(1));
    let mut retry = observation()?;
    retry.confirmed = true;
    assert_eq!(
        service
            .record_walking_observation("alice", retry)
            .await?
            .sample_count,
        Some(1)
    );
    let mut foreign = observation()?;
    foreign.confirmed = true;
    assert!(
        service
            .record_walking_observation("bob", foreign)
            .await
            .is_err()
    );
    let mut conflict = observation()?;
    conflict.confirmed = true;
    conflict.elapsed_minutes = 15.0;
    assert!(
        service
            .record_walking_observation("alice", conflict)
            .await
            .is_err()
    );
    let mut second = observation()?;
    second.confirmed = true;
    second.observation_id = "walk-test-2".into();
    second.distance_meters = 500.0;
    second.elapsed_minutes = 10.0;
    let result = service.record_walking_observation("alice", second).await?;
    assert_eq!(result.sample_count, Some(2));
    assert!(
        (result.profile_speed_kmh.ok_or("missing weighted speed")? - 1500.0 * 0.06 / 22.0).abs()
            < 0.000001
    );
    let mut legacy = routine()?;
    legacy.id = "work".into();
    legacy.walking_speed_kmh = None;
    service.store.upsert("alice", &legacy).await?;
    let retained = service
        .store
        .get("alice", "work")
        .await?
        .walking_speed_kmh
        .ok_or("missing retained speed")?;
    assert!((retained - result.profile_speed_kmh.ok_or("missing profile speed")?).abs() < 0.000001);
    Ok(())
}
