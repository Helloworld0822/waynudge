use crate::error::{Error, Result, invalid};
use chrono::{DateTime, NaiveDate, Utc};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
#[derive(Clone)]
pub(super) struct LiveHttp {
    inner: Arc<Client>,
}
struct Client {
    http: reqwest::Client,
    limit: u32,
    state: Mutex<State>,
    cache_ttl: Duration,
    max_bytes: usize,
}
struct State {
    day: NaiveDate,
    used: u32,
    cache: HashMap<String, (Instant, Fetched)>,
}
#[derive(Clone)]
pub(super) struct Fetched {
    pub bytes: Vec<u8>,
    pub fetched_at: DateTime<Utc>,
}
impl LiveHttp {
    pub fn new(limit: u32) -> Result<Self> {
        Self::configured(limit, Duration::from_secs(15), 524_288)
    }
    pub fn historical(limit: u32) -> Result<Self> {
        Self::configured(limit, Duration::from_secs(3600), 4_194_304)
    }
    fn configured(limit: u32, cache_ttl: Duration, max_bytes: usize) -> Result<Self> {
        if !(1..=100_000).contains(&limit) {
            return Err(invalid("live API daily limit must be 1..100000"));
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Provider)?;
        Ok(Self {
            inner: Arc::new(Client {
                http,
                limit,
                cache_ttl,
                max_bytes,
                state: Mutex::new(State {
                    day: Utc::now().date_naive(),
                    used: 0,
                    cache: HashMap::new(),
                }),
            }),
        })
    }
    pub async fn get(&self, url: reqwest::Url, cache_key: String) -> Result<Vec<u8>> {
        Ok(self.get_with_timestamp(url, cache_key).await?.bytes)
    }
    pub async fn get_with_timestamp(
        &self,
        url: reqwest::Url,
        cache_key: String,
    ) -> Result<Fetched> {
        tokio::time::timeout(Duration::from_secs(10), self.fetch(url, cache_key))
            .await
            .map_err(|_| Error::Provider)?
    }
    async fn fetch(&self, url: reqwest::Url, cache_key: String) -> Result<Fetched> {
        let mut state = self.inner.state.lock().await;
        let today = Utc::now().date_naive();
        if state.day != today {
            state.day = today;
            state.used = 0;
            state.cache.clear();
        }
        state
            .cache
            .retain(|_, (at, _)| at.elapsed() < self.inner.cache_ttl);
        if let Some((_, bytes)) = state.cache.get(&cache_key) {
            return Ok(bytes.clone());
        }
        if state.used >= self.inner.limit {
            return Err(Error::Provider);
        }
        state.used = state.used.saturating_add(1);
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
            if bytes.len().saturating_add(chunk.len()) > self.inner.max_bytes {
                return Err(Error::Provider);
            }
            bytes.extend_from_slice(&chunk);
        }
        if state.cache.len() >= 64 {
            state.cache.clear();
        }
        let fetched = Fetched {
            bytes,
            fetched_at: Utc::now(),
        };
        state
            .cache
            .insert(cache_key, (Instant::now(), fetched.clone()));
        Ok(fetched)
    }
}
