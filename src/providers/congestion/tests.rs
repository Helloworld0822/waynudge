use super::*;
use crate::providers::seoul::Direction;
use serde_json::{Value, json};
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
fn query() -> CongestionQuery {
    CongestionQuery {
        station_name: "서울".into(),
        line: 1,
        day_type: DayType::Weekday,
        time: "08:45".into(),
        direction: Some(Direction::Up),
    }
}
fn row() -> Value {
    // Official 2026-06-30 dataset: Seoul Station, weekday, line 1, up.
    json!({"구분":"평일","호선":"1호선","역번호":150,"역명":"서울역","상하구분":"상선","8시30분":"92.20","00시30분":"1.10"})
}
fn payload(rows: Vec<Value>) -> Value {
    json!({"data":rows,"currentCount":rows.len(),"matchCount":rows.len(),"totalCount":rows.len(),"page":1})
}
#[test]
fn historical_average_filters_station_line_day_direction_and_half_hour() -> TestResult {
    let mut other_line = row();
    other_line["호선"] = json!("4호선");
    let mut other_day = row();
    other_day["구분"] = json!("토요일");
    let mut other_direction = row();
    other_direction["상하구분"] = json!("하선");
    let bytes = serde_json::to_vec(&payload(vec![
        row(),
        other_line,
        other_day,
        other_direction,
    ]))?;
    let result = wire::parse(&bytes, &query())?;
    assert_eq!(result.observations.len(), 1);
    assert_eq!(result.observations[0].congestion_percent, Some(92.2));
    assert_eq!(result.time_slot_start, "08:30");
    assert_eq!(result.time_slot_end, "09:00");
    assert_eq!(result.dataset_as_of, "2026-06-30");
    assert!(!result.realtime);
    assert_eq!(result.source, "seoul_metro_odcloud");
    Ok(())
}
#[test]
fn midnight_and_unknown_values_are_not_fabricated() -> TestResult {
    let mut q = query();
    q.time = "00:45".into();
    let bytes = serde_json::to_vec(&payload(vec![row()]))?;
    assert_eq!(
        wire::parse(&bytes, &q)?.observations[0].congestion_percent,
        Some(1.1)
    );
    for value in [json!(null), json!(""), json!("-"), json!("-1")] {
        let mut r = row();
        r["8시30분"] = value;
        let result = wire::parse(&serde_json::to_vec(&payload(vec![r]))?, &query())?;
        assert_eq!(result.observations[0].congestion_percent, None);
        assert_eq!(result.warnings.len(), 2);
    }
    let mut r = row();
    r["8시30분"] = json!("0.00");
    assert_eq!(
        wire::parse(&serde_json::to_vec(&payload(vec![r]))?, &query())?.observations[0]
            .congestion_percent,
        Some(0.0)
    );
    q.station_name = "없는역".into();
    assert!(wire::parse(&bytes, &q)?.observations.is_empty());
    Ok(())
}
#[test]
fn validation_upstream_errors_and_truncation_are_rejected() -> TestResult {
    let mut q = query();
    q.line = 9;
    assert!(q.validate().is_err());
    q = query();
    q.time = "25:00".into();
    assert!(q.validate().is_err());
    assert!(
        serde_json::from_value::<CongestionQuery>(
            json!({"station_name":"서울역","line":1,"day_type":"holiday","time":"08:30"})
        )
        .is_err()
    );
    assert!(wire::parse(br#"{"code":-4,"msg":"upstream secret"}"#, &query()).is_err());
    let mut incomplete = payload(vec![row()]);
    incomplete["totalCount"] = json!(1671);
    assert!(wire::parse(&serde_json::to_vec(&incomplete)?, &query()).is_err());
    let mut r = row();
    r["8시30분"] = json!("NaN");
    assert!(wire::parse(&serde_json::to_vec(&payload(vec![r]))?, &query()).is_err());
    Ok(())
}
#[tokio::test]
async fn cached_provider_is_callable_through_public_mcp_and_authenticated_http() -> TestResult {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
        routing::get,
    };
    use std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use tower::ServiceExt;
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let upstream = axum::Router::new().route(
        "/",
        get(move |uri: axum::http::Uri| {
            let calls = calls.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                let url = reqwest::Url::parse(&format!("http://localhost{uri}"));
                let valid = url.is_ok_and(|url| {
                    url.query_pairs()
                        .any(|(k, v)| k == "serviceKey" && v == "test+key/=")
                });
                if valid {
                    (StatusCode::OK, axum::Json(payload(vec![row()])))
                } else {
                    (StatusCode::UNAUTHORIZED, axum::Json(json!({"code":-4})))
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await });
    let mut client = SubwayCongestion::new(" test%2Bkey%2F%3D \n".into(), 1)?;
    client.endpoint = reqwest::Url::parse(&format!("http://{address}/"))?;
    let mut provider = crate::providers::Provider::mock();
    provider.congestion = Some(client);
    let service = crate::service::Service {
        provider,
        store: crate::storage::Store::open("sqlite::memory:").await?,
    };
    let token = "congestion-owner-token-at-least-32-characters";
    let auth = crate::api::Auth::new(HashMap::from([("owner".into(), token.into())]), None)?
        .with_public_mcp(true);
    let app = crate::api::router(service, auth);
    let call = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_subway_congestion","arguments":query()}});
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(serde_json::to_vec(&call)?))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await?)?;
    assert_eq!(body["result"]["structuredContent"]["realtime"], false);
    assert_eq!(
        body["result"]["structuredContent"]["observations"][0]["congestion_percent"],
        92.2
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/subway/congestion")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(serde_json::to_vec(&query())?))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await?)?;
    assert_eq!(body["observations"][0]["congestion_percent"], 92.2);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server.abort();
    Ok(())
}
#[tokio::test]
async fn upstream_http_failure_is_redacted_and_never_becomes_mock() -> TestResult {
    let app = axum::Router::new().route(
        "/",
        axum::routing::get(|| async {
            (axum::http::StatusCode::UNAUTHORIZED, "secret service key")
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let mut client = SubwayCongestion::new("secret-service-key".into(), 1)?;
    client.endpoint = reqwest::Url::parse(&format!("http://{address}/"))?;
    assert!(matches!(
        client.congestion(&query()).await,
        Err(Error::Provider)
    ));
    assert!(matches!(
        client.congestion(&query()).await,
        Err(Error::Provider)
    ));
    server.abort();
    Ok(())
}
