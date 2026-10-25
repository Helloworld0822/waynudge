use std::{collections::HashMap, env};
use waynudge::{
    api::{self, Auth},
    providers::Provider,
    service::Service,
    storage::Store,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .init();
    let users: HashMap<String, String> =
        serde_json::from_str(&env::var("WAYNUDGE_USERS").map_err(|_| {
            anyhow::anyhow!(
                "set WAYNUDGE_USERS JSON map of user IDs to unique tokens (32+ characters)"
            )
        })?)
        .map_err(|_| anyhow::anyhow!("WAYNUDGE_USERS must be a JSON map"))?;
    let auth = Auth::new(users, env::var("PUBLIC_HOST").ok())?
        .with_public_mcp(env::var("PUBLIC_MCP").is_ok_and(|value| value == "true"));
    let provider = match env::var("TRANSIT_PROVIDER")
        .unwrap_or_else(|_| "mock".into())
        .as_str()
    {
        "mock" => Provider::mock_with_delay(
            env::var("MOCK_SUBWAY_DELAY_MINUTES")
                .unwrap_or_else(|_| "0".into())
                .parse()?,
        )?,
        "odsay" => Provider::odsay(
            env::var("ODSAY_API_KEY").map_err(|_| anyhow::anyhow!("ODSAY_API_KEY required"))?,
            env::var("ODSAY_DAILY_LIMIT")
                .unwrap_or_else(|_| "30".into())
                .parse()?,
            env::var("ODSAY_CACHE_SECONDS")
                .unwrap_or_else(|_| "0".into())
                .parse()?,
        )?,
        _ => anyhow::bail!("TRANSIT_PROVIDER must be mock or odsay"),
    };
    let provider = match env::var("SEOUL_SUBWAY_API_KEY") {
        Ok(key) if !key.trim().is_empty() => provider.with_seoul(
            key,
            env::var("SEOUL_SUBWAY_DAILY_LIMIT")
                .unwrap_or_else(|_| "1000".into())
                .parse()?,
        )?,
        Ok(_) | Err(_) => provider,
    };
    let mut provider = provider;
    if let Ok(key) = env::var("SUBWAY_CONGESTION_SERVICE_KEY")
        && !key.trim().is_empty()
    {
        provider = provider.with_subway_congestion(
            key,
            env::var("SUBWAY_CONGESTION_DAILY_LIMIT")
                .unwrap_or_else(|_| "100".into())
                .parse()?,
        )?;
    }
    let tago_key = env::var("TAGO_SERVICE_KEY").unwrap_or_default();
    let gyeonggi_key = env::var("GYEONGGI_SERVICE_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .unwrap_or_else(|| tago_key.clone());
    for (kind, key) in [
        (waynudge::providers::bus::BusKind::Tago, tago_key),
        (waynudge::providers::bus::BusKind::Gyeonggi, gyeonggi_key),
    ] {
        if !key.trim().is_empty() {
            provider = provider.with_bus(
                kind,
                key,
                env::var("BUS_DAILY_LIMIT")
                    .unwrap_or_else(|_| "1000".into())
                    .parse()?,
            )?;
        }
    }
    let service = Service {
        store: Store::open(
            &env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://waynudge.db".into()),
        )
        .await?,
        provider,
    };
    let bind: std::net::SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let app = api::router(service.clone(), auth);
    tracing::info!(address=%bind,"WayNudge listening");
    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown());
    server.await?;
    Ok(())
}
async fn shutdown() {
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => {
                tracing::warn!("SIGTERM handler unavailable");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {result=tokio::signal::ctrl_c()=>{if result.is_err(){tracing::warn!("interrupt handler failed");}},()=terminate=>{}}
}
