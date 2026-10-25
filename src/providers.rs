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
pub mod bus;
mod live;
mod live_http;
pub mod seoul;
use bus::{BusArrivals, BusKind, BusProvider, BusQuery};
use seoul::{SeoulSubway, SubwayArrivals, SubwayQuery};

const ENDPOINT: &str = "https://api.odsay.com/v1/api/searchPubTransPathT";
#[derive(Clone)]
pub struct Provider {
    external: Option<Arc<ODsay>>,
    mock_delay_minutes: u32,
    seoul: Option<SeoulSubway>,
    tago: Option<BusProvider>,
    gyeonggi: Option<BusProvider>,
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
            seoul: None,
            tago: None,
            gyeonggi: None,
        }
    }
    pub fn mock_with_delay(minutes: u32) -> Result<Self> {
        if minutes > 180 {
            return Err(invalid("mock delay must be 0..180 minutes"));
        }
        Ok(Self {
            external: None,
            mock_delay_minutes: minutes,
            seoul: None,
            tago: None,
            gyeonggi: None,
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
            seoul: None,
            tago: None,
            gyeonggi: None,
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
        let mut routes = match &self.external {
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
                    subway_boarding: match mode {
                        Mode::Subway | Mode::Mixed => Some(seoul::SubwayBoarding {
                            access_minutes: walk_minutes,
                            station_name: "Mock subway station".into(),
                            line_id: Some("1001".into()),
                            direction: Some(seoul::Direction::Up),
                            live: None,
                        }),
                        Mode::Bus | Mode::Walk => None,
                    },
                })
                .collect(),
            }),
            Some(provider) => tokio::time::timeout(
                Duration::from_secs(10),
                provider.routes(origin, destination),
            )
            .await
            .map_err(|_| Error::Provider)?,
        }?;
        if let Some(seoul) = &self.seoul
            && routes.source == "odsay"
        {
            for route in &mut routes.routes {
                let Some(boarding) = &mut route.subway_boarding else {
                    continue;
                };
                if boarding.line_id.is_none() || boarding.direction.is_none() {
                    continue;
                }
                let query = SubwayQuery {
                    station_name: boarding.station_name.clone(),
                    line_id: boarding.line_id.clone(),
                    direction: boarding.direction,
                };
                match seoul.arrivals(&query).await {
                    Ok(arrivals) => boarding.live = Some(arrivals),
                    Err(_) => routes.warnings.push(
                        "Seoul subway real-time lookup failed; scheduled route estimates retained."
                            .into(),
                    ),
                }
            }
        }
        Ok(routes)
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
#[path = "providers/tests.rs"]
mod tests;
