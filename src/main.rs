use std::{
    collections::{HashMap, HashSet},
    env,
    time::Duration,
};
use waynudge::{
    api::{self, Auth},
    providers::Provider,
    routines::date,
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
    let auth = Auth::new(users, env::var("PUBLIC_HOST").ok())?;
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
    let holidays: HashSet<String> = env::var("HOLIDAYS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    for holiday in &holidays {
        date(holiday)?;
    }
    let service = Service {
        store: Store::open(
            &env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://waynudge.db".into()),
        )
        .await?,
        provider,
        holidays,
    };
    let bind: std::net::SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let app = api::router(service.clone(), auth);
    tracing::info!(address=%bind,"WayNudge listening");
    let scheduler = async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        let mut checked = HashMap::new();
        loop {
            interval.tick().await;
            if service.scheduler_tick(&mut checked).await.is_err() {
                tracing::warn!("scheduler unavailable");
            }
        }
    };
    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown());
    tokio::select! {result=server=>result?,()=scheduler=>{}}
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
