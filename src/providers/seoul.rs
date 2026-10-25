mod model;
use crate::error::{Error, Result, invalid};
use chrono::{NaiveDate, Utc};
pub use model::{Direction, SubwayArrival, SubwayArrivals, SubwayBoarding, SubwayQuery};
use model::{Payload, normalize, parse};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

// Seoul publishes this endpoint over HTTP; redirects are disabled and request URLs are never logged.
const BASE: &str = "http://swopenapi.seoul.go.kr";
#[derive(Clone)]
pub struct SeoulSubway {
    inner: Arc<Client>,
}
struct Client {
    key: String,
    http: reqwest::Client,
    daily_limit: u32,
    state: Mutex<State>,
    base: reqwest::Url,
}
struct State {
    day: NaiveDate,
    used: u32,
    cache: HashMap<String, (Instant, Vec<u8>)>,
}
impl SeoulSubway {
    pub fn new(key: String, daily_limit: u32) -> Result<Self> {
        if key.trim().is_empty()
            || key.len() > 256
            || !key.bytes().all(|c| c.is_ascii_alphanumeric())
            || !(1..=100_000).contains(&daily_limit)
        {
            return Err(invalid("invalid Seoul subway key or daily request limit"));
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Provider)?;
        let base = reqwest::Url::parse(BASE).map_err(|_| Error::Provider)?;
        Ok(Self {
            inner: Arc::new(Client {
                key,
                http,
                daily_limit,
                state: Mutex::new(State {
                    day: Utc::now().date_naive(),
                    used: 0,
                    cache: HashMap::new(),
                }),
                base,
            }),
        })
    }
    pub async fn arrivals(&self, query: &SubwayQuery) -> Result<SubwayArrivals> {
        query.validate()?;
        tokio::time::timeout(Duration::from_secs(10), self.fetch(query))
            .await
            .map_err(|_| Error::Provider)?
    }
    async fn fetch(&self, query: &SubwayQuery) -> Result<SubwayArrivals> {
        let station = normalize(&query.station_name).to_string();
        let mut state = self.inner.state.lock().await;
        let today = Utc::now().date_naive();
        if state.day != today {
            state.day = today;
            state.used = 0;
            state.cache.clear();
        }
        state
            .cache
            .retain(|_, (at, _)| at.elapsed() < Duration::from_secs(15));
        if let Some((_, bytes)) = state.cache.get(&station) {
            return parse(
                serde_json::from_slice::<Payload>(bytes).map_err(|_| Error::Provider)?,
                query,
                Utc::now(),
            );
        }
        if state.used >= self.inner.daily_limit {
            return Err(Error::Provider);
        }
        state.used = state.used.saturating_add(1);
        let mut url = self.inner.base.clone();
        url.path_segments_mut()
            .map_err(|_| Error::Provider)?
            .extend([
                "api",
                "subway",
                &self.inner.key,
                "json",
                "realtimeStationArrival",
                "0",
                "100",
                &station,
            ]);
        let mut response = self
            .inner
            .http
            .get(url)
            .send()
            .await
            .map_err(|_| Error::Provider)?
            .error_for_status()
            .map_err(|_| Error::Provider)?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Provider)? {
            if bytes.len().saturating_add(chunk.len()) > 524_288 {
                return Err(Error::Provider);
            }
            bytes.extend_from_slice(&chunk);
        }
        let result = parse(
            serde_json::from_slice::<Payload>(&bytes).map_err(|_| Error::Provider)?,
            query,
            Utc::now(),
        )?;
        if state.cache.len() >= 64 {
            state.cache.clear();
        }
        state.cache.insert(station, (Instant::now(), bytes));
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
