use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use tower::ServiceExt;
use waynudge::{
    api::{self, Auth},
    providers::Provider,
    routines::Routine,
    service::{BriefRequest, Service, UpsertRequest},
    storage::{DayStatus, Store},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ALICE: &str = "alice-secret-with-more-than-32-characters";
const BOB: &str = "bob-secret-with-more-than-32-characters";
fn routine() -> Result<Routine, serde_json::Error> {
    serde_json::from_value(
        json!({"id":"work","name":"Office","origin":{"name":"Home","lat":37.5,"lon":127.0},"destination":{"name":"Work","lat":37.6,"lon":127.1},"arrival_time":"09:00","weekdays":[1,2,3,4,5,6,7],"preferred_modes":["subway","bus","mixed"],"max_transfers":2,"max_walk_minutes":20,"buffer_minutes":5,"preparation_minutes":10,"min_alert_interval_minutes":10,"walking_factor":1.0,"usual_route_id":null}),
    )
}
async fn app() -> Result<(Router, Service), Box<dyn std::error::Error>> {
    let service = Service {
        store: Store::open("sqlite::memory:").await?,
        provider: Provider::mock(),
        holidays: HashSet::new(),
    };
    let auth = Auth::new(
        HashMap::from([("alice".into(), ALICE.into()), ("bob".into(), BOB.into())]),
        None,
    )?;
    Ok((api::router(service.clone(), auth), service))
}
async fn request(
    app: Router,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
    let mut builder = Request::builder().uri(path).header("host", "localhost");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => builder
            .method("POST")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-03-26")
            .body(Body::from(serde_json::to_vec(&body)?))?,
        None => builder.body(Body::empty())?,
    };
    let response = app.oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_048_576).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
    ))
}
#[tokio::test]
async fn authentication_and_confirmation_protect_http_data() -> TestResult {
    // Given
    let (app, _) = app().await?;
    let body = json!({"routine":routine()?,"confirmed":false});
    // When / Then
    assert_eq!(
        request(app.clone(), "/health", None, None).await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            app.clone(),
            "/api/routines",
            Some(ALICE),
            Some(body.clone())
        )
        .await?
        .0,
        StatusCode::CONFLICT
    );
    let mut body = body;
    body["confirmed"] = json!(true);
    assert_eq!(
        request(app.clone(), "/api/routines", Some(ALICE), Some(body))
            .await?
            .0,
        StatusCode::OK
    );
    assert_eq!(
        request(app.clone(), "/api/routines", Some(BOB), None)
            .await?
            .1["routines"],
        json!([])
    );
    assert_eq!(
        request(
            app,
            "/api/brief",
            Some(BOB),
            Some(json!({"routine_id":"work","date":"2026-10-26"}))
        )
        .await?
        .0,
        StatusCode::NOT_FOUND
    );
    Ok(())
}
#[tokio::test]
async fn seven_tools_are_callable_over_stateless_json_http() -> TestResult {
    // Given: one stateless request per tool, real SDK transport.
    let (app, _) = app().await?;
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}});
    let (status, result) = request(app.clone(), "/mcp", Some(ALICE), Some(init)).await?;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(
        result["result"]["capabilities"]["tools"].is_object(),
        "{result}"
    );
    let (status, result) = request(
        app.clone(),
        "/mcp",
        Some(ALICE),
        Some(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        result["result"]["tools"]
            .as_array()
            .ok_or("missing tools")?
            .len(),
        7
    );
    let calls = [
        (
            "upsert_routine",
            json!({"routine":routine()?,"confirmed":true}),
        ),
        ("list_routines", json!({})),
        (
            "get_commute_brief",
            json!({"routine_id":"work","date":"2026-10-26"}),
        ),
        (
            "compare_routes",
            json!({"routine_id":"work","date":"2026-10-26"}),
        ),
        (
            "replan_trip",
            json!({"routine_id":"work","date":"2026-10-26","current_location":{"name":"Station","lat":37.55,"lon":127.05},"stage":"at_station","blocked_route_ids":[]}),
        ),
        (
            "set_routine_day_status",
            json!({"routine_id":"work","date":"2026-10-26","enabled":false,"confirmed":true}),
        ),
        (
            "record_trip_feedback",
            json!({"routine_id":"work","route_id":"mock-subway","actual_minutes":40,"walking_factor":1.2,"confirmed":true}),
        ),
    ];
    // When / Then: every call has structured JSON, never an SSE stream.
    for (name, args) in calls {
        let payload = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":name,"arguments":args}});
        let (status, result) = request(app.clone(), "/mcp", Some(ALICE), Some(payload)).await?;
        assert_eq!(status, StatusCode::OK, "{name}: {result}");
        assert!(result.get("error").is_none(), "{name}: {result}");
        assert!(
            result["result"]["structuredContent"].is_object(),
            "{name}: {result}"
        );
        assert_ne!(result["result"]["isError"], true, "{name}: {result}");
    }
    let payload = json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_routines","arguments":{}}});
    let result = request(app, "/mcp", Some(BOB), Some(payload)).await?.1;
    assert_eq!(result["result"]["structuredContent"]["routines"], json!([]));
    Ok(())
}

#[path = "http_mcp/service_checks.rs"]
mod service_checks;
