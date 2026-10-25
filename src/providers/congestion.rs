mod model;
mod wire;
use super::live_http::LiveHttp;
use crate::error::{Error, Result, invalid};
pub use model::{CongestionQuery, DayType, SubwayCongestionReport};

// Pinned to the latest quarter published when this adapter was verified.
const ENDPOINT: &str =
    "https://api.odcloud.kr/api/15071311/v1/uddi:ec6e85c3-f01a-415d-8705-e20650f1be84";
const DATASET_AS_OF: &str = "2026-06-30";
#[derive(Clone)]
pub struct SubwayCongestion {
    key: String,
    http: LiveHttp,
    endpoint: reqwest::Url,
}
impl SubwayCongestion {
    pub fn new(key: String, daily_limit: u32) -> Result<Self> {
        let key = key.trim();
        if key.is_empty() || key.len() > 512 || !key.bytes().all(|c| c.is_ascii_graphic()) {
            return Err(invalid("invalid subway congestion service key"));
        }
        let key = if key.contains('%') {
            let url = reqwest::Url::parse(&format!("https://localhost/?serviceKey={key}"))
                .map_err(|_| invalid("invalid encoded subway congestion service key"))?;
            let mut pairs = url.query_pairs();
            let (_, key) = pairs.next().ok_or_else(|| invalid("missing service key"))?;
            if pairs.next().is_some() {
                return Err(invalid("invalid encoded subway congestion service key"));
            }
            key.into_owned()
        } else {
            key.to_string()
        };
        Ok(Self {
            key,
            http: LiveHttp::historical(daily_limit)?,
            endpoint: reqwest::Url::parse(ENDPOINT).map_err(|_| Error::Provider)?,
        })
    }
    pub async fn congestion(&self, query: &CongestionQuery) -> Result<SubwayCongestionReport> {
        query.validate()?;
        let mut url = self.endpoint.clone();
        url.query_pairs_mut()
            .append_pair("serviceKey", &self.key)
            .append_pair("page", "1")
            .append_pair("perPage", "2000")
            .append_pair("returnType", "JSON");
        let bytes = self.http.get(url, DATASET_AS_OF.into()).await?;
        wire::parse(&bytes, query)
    }
}
#[cfg(test)]
mod tests;
