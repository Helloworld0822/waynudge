use crate::{
    error::{Error, Result, invalid},
    routines::{Mode, Place},
    routing::{Route, RouteSet},
};
use chrono::{NaiveDate, Utc};
#[path = "providers/odsay.rs"]
mod odsay;
use odsay::{Payload, parse};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const ENDPOINT: &str = "https://api.odsay.com/v1/api/searchPubTransPathT";
#[derive(Clone)]
pub struct Provider {
    external: Option<Arc<ODsay>>,
    mock_delay_minutes: u32,
}
struct ODsay {
    key: String,
    client: reqwest::Client,
    daily_limit: u32,
    cache_seconds: u64,
    state: Mutex<State>,
    #[cfg(test)]
    endpoint: String,
}
struct State {
    day: NaiveDate,
    used: u32,
    cache: HashMap<String, (Instant, RouteSet)>,
}
impl Provider {
    pub fn mock() -> Self {
        Self {
            external: None,
            mock_delay_minutes: 0,
        }
    }
    pub fn mock_with_delay(minutes: u32) -> Result<Self> {
        if minutes > 180 {
            return Err(invalid("mock delay must be 0..180 minutes"));
        }
        Ok(Self {
            external: None,
            mock_delay_minutes: minutes,
        })
    }
    pub fn odsay(api_key: String, daily_limit: u32, cache_seconds: u64) -> Result<Self> {
        if api_key.trim().is_empty() || !(1..=30).contains(&daily_limit) || cache_seconds > 300 {
            return Err(invalid(
                "provider requires a key, daily limit 1..30 and cache TTL 0..300 seconds",
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Provider)?;
        Ok(Self {
            mock_delay_minutes: 0,
            external: Some(Arc::new(ODsay {
                key: api_key,
                client,
                daily_limit,
                cache_seconds,
                state: Mutex::new(State {
                    day: Utc::now().date_naive(),
                    used: 0,
                    cache: HashMap::new(),
                }),
                #[cfg(test)]
                endpoint: ENDPOINT.into(),
            })),
        })
    }
    pub async fn routes(&self, origin: &Place, destination: &Place) -> Result<RouteSet> {
        origin.validate()?;
        destination.validate()?;
        match &self.external {
            None => Ok(RouteSet {
                source: "mock".into(),
                warnings: vec![
                    "Synthetic demo routes; not real transit or live information.".into(),
                    format!(
                        "Simulated subway delay: {} minutes; not an observed disruption.",
                        self.mock_delay_minutes
                    ),
                ],
                routes: [
                    ("mock-subway", Mode::Subway, 38, 8, 1),
                    ("mock-bus", Mode::Bus, 44, 4, 0),
                    ("mock-mixed", Mode::Mixed, 35, 10, 2),
                ]
                .into_iter()
                .map(|(id, mode, total_minutes, walk_minutes, transfers)| Route {
                    id: id.into(),
                    mode,
                    total_minutes,
                    walk_minutes,
                    transfers,
                    cost_won: 1500,
                    collected_at: Utc::now(),
                    realtime: false,
                    delay_minutes: if id == "mock-subway" {
                        self.mock_delay_minutes
                    } else {
                        0
                    },
                    origin: origin.clone(),
                    destination: destination.clone(),
                })
                .collect(),
            }),
            Some(provider) => tokio::time::timeout(
                Duration::from_secs(10),
                provider.routes(origin, destination),
            )
            .await
            .map_err(|_| Error::Provider)?,
        }
    }
}
impl ODsay {
    async fn routes(&self, origin: &Place, destination: &Place) -> Result<RouteSet> {
        let key = format!(
            "{}:{}:{}:{}:{}:{}",
            origin.lat, origin.lon, destination.lat, destination.lon, origin.name, destination.name
        );
        let mut state = self.state.lock().await;
        let today = Utc::now().date_naive();
        if state.day != today {
            state.day = today;
            state.used = 0;
            state.cache.clear();
        }
        state
            .cache
            .retain(|_, (at, _)| at.elapsed() < Duration::from_secs(self.cache_seconds));
        if let Some((_, routes)) = state.cache.get(&key) {
            return Ok(routes.clone());
        }
        if state.used >= self.daily_limit {
            return Err(Error::Provider);
        }
        state.used += 1;
        #[cfg(not(test))]
        let endpoint = ENDPOINT;
        #[cfg(test)]
        let endpoint = &self.endpoint;
        let response = self
            .client
            .get(endpoint)
            .query(&[
                ("apiKey", self.key.clone()),
                ("SX", origin.lon.to_string()),
                ("SY", origin.lat.to_string()),
                ("EX", destination.lon.to_string()),
                ("EY", destination.lat.to_string()),
                ("OPT", "0".into()),
            ])
            .send()
            .await
            .map_err(|_| Error::Provider)?
            .error_for_status()
            .map_err(|_| Error::Provider)?;
        let mut response = response;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Provider)? {
            if body.len().saturating_add(chunk.len()) > 1_048_576 {
                return Err(Error::Provider);
            }
            body.extend_from_slice(&chunk);
        }
        let payload: Payload = serde_json::from_slice(&body).map_err(|_| Error::Provider)?;
        let routes = parse(payload, origin, destination)?;
        if self.cache_seconds > 0 {
            if state.cache.len() >= 64 {
                state.cache.clear();
            }
            state.cache.insert(key, (Instant::now(), routes.clone()));
        }
        Ok(routes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn place() -> Place {
        Place {
            name: "test".into(),
            lat: 37.5,
            lon: 127.0,
        }
    }
    fn fixture(minutes: u32, traffic: u32) -> String {
        format!(
            r#"{{"result":{{"path":[{{"info":{{"totalTime":{minutes},"payment":1500,"totalWalk":300}},"subPath":[{{"trafficType":3,"sectionTime":5,"distance":300}},{{"trafficType":{traffic},"sectionTime":25,"distance":5000,"startName":"A","endName":"B","lane":[{{"name":"Line 2","subwayCode":2}}]}}]}}]}}}}"#
        )
    }
    #[test]
    fn parses_estimates_with_stable_identity() -> Result<()> {
        let origin = place();
        let a = parse(serde_json::from_str(&fixture(30, 1))?, &origin, &origin)?;
        let b = parse(serde_json::from_str(&fixture(40, 1))?, &origin, &origin)?;
        assert_eq!(a.routes[0].id, b.routes[0].id);
        assert_eq!(a.routes[0].walk_minutes, 5);
        assert_eq!(a.routes[0].transfers, 0);
        assert!(!a.routes[0].realtime);
        assert!(parse(serde_json::from_str(&fixture(30, 4))?, &origin, &origin).is_err());
        Ok(())
    }
    #[tokio::test]
    async fn http_failure_is_private_and_consumes_quota() -> Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| Error::Provider)?;
        let address = listener.local_addr().map_err(|_| Error::Provider)?;
        let server = tokio::spawn(async move {
            let app = axum::Router::new().route(
                "/",
                axum::routing::get(|| async {
                    (
                        axum::http::StatusCode::BAD_GATEWAY,
                        "secret-key coordinates",
                    )
                }),
            );
            axum::serve(listener, app).await
        });
        let mut provider = Provider::odsay("secret-key".into(), 1, 0)?;
        if let Some(external) = provider.external.as_mut().and_then(Arc::get_mut) {
            external.endpoint = format!("http://{address}/");
        }
        let result = provider.routes(&place(), &place()).await;
        assert!(matches!(result, Err(Error::Provider)));
        if let Some(external) = &provider.external {
            assert_eq!(external.state.lock().await.used, 1);
        }
        assert!(matches!(
            provider.routes(&place(), &place()).await,
            Err(Error::Provider)
        ));
        server.abort();
        Ok(())
    }
}
