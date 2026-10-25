mod auth;
use crate::{
    alerts::Alert,
    error::Result,
    mcp::Mcp,
    routing::Comparison,
    service::{
        Brief, BriefRequest, MutationResult, ReplanRequest, RoutineList, Service, UpsertRequest,
    },
    storage::{DayStatus, Feedback},
};
pub use auth::{Auth, User};
use axum::{
    Extension, Json, Router,
    extract::State,
    middleware,
    routing::{get, post},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use std::{sync::Arc, time::Duration};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer};

pub fn router(service: Service, auth: Auth) -> Router {
    let handler = Mcp::new(service.clone());
    let mut config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true);
    if let Some(host) = &auth.public_host {
        config = config.with_allowed_hosts([
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            host.clone(),
        ]);
    }
    let mcp = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    Router::new()
        .route("/health", get(health))
        .route("/api/routines", get(list).post(upsert))
        .route("/api/brief", post(brief))
        .route("/api/compare", post(compare))
        .route("/api/replan", post(replan))
        .route("/api/day-status", post(day_status))
        .route("/api/feedback", post(feedback))
        .route("/api/alerts", get(alerts))
        .route("/api/alerts/evaluate", post(evaluate))
        .nest_service("/mcp", mcp)
        .with_state(service)
        .layer(RequestBodyLimitLayer::new(65_536))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(middleware::from_fn_with_state(auth, auth::authenticate))
}
async fn health(State(s): State<Service>) -> Result<Json<serde_json::Value>> {
    s.store.health().await?;
    Ok(Json(serde_json::json!({"status":"ok"})))
}
async fn list(
    State(s): State<Service>,
    Extension(u): Extension<User>,
) -> Result<Json<RoutineList>> {
    s.list(&u.0).await.map(Json)
}
async fn upsert(
    State(s): State<Service>,
    Extension(u): Extension<User>,
    Json(i): Json<UpsertRequest>,
) -> Result<Json<MutationResult>> {
    s.upsert(&u.0, i).await.map(Json)
}
async fn brief(
    State(s): State<Service>,
    Extension(u): Extension<User>,
    Json(i): Json<BriefRequest>,
) -> Result<Json<Brief>> {
    s.brief(&u.0, i).await.map(Json)
}
async fn compare(
    State(s): State<Service>,
    Extension(u): Extension<User>,
    Json(i): Json<BriefRequest>,
) -> Result<Json<Comparison>> {
    s.compare(&u.0, i).await.map(Json)
}
async fn replan(
    State(s): State<Service>,
    Extension(u): Extension<User>,
    Json(i): Json<ReplanRequest>,
) -> Result<Json<Brief>> {
    s.replan(&u.0, i).await.map(Json)
}
async fn day_status(
    State(s): State<Service>,
    Extension(u): Extension<User>,
    Json(i): Json<DayStatus>,
) -> Result<Json<MutationResult>> {
    s.set_day(&u.0, i).await.map(Json)
}
async fn feedback(
    State(s): State<Service>,
    Extension(u): Extension<User>,
    Json(i): Json<Feedback>,
) -> Result<Json<MutationResult>> {
    s.feedback(&u.0, i).await.map(Json)
}
async fn alerts(
    State(s): State<Service>,
    Extension(u): Extension<User>,
) -> Result<Json<Vec<Alert>>> {
    s.store.alerts(&u.0).await.map(Json)
}
async fn evaluate(
    State(s): State<Service>,
    Extension(u): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    Ok(Json(
        serde_json::json!({"enqueued":s.evaluate_alerts(&u.0,chrono::Utc::now()).await?}),
    ))
}
