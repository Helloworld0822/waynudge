mod discovery;
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
    auto_discovery: bool,
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
            auto_discovery: false,
            key,
            http: LiveHttp::historical(daily_limit)?,
            endpoint: reqwest::Url::parse(ENDPOINT).map_err(|_| Error::Provider)?,
        })
    }
    pub(super) fn with_environment(mut self) -> Result<Self> {
        let value = std::env::var("SUBWAY_CONGESTION_AUTO_DISCOVERY")
            .map(Some)
            .or_else(|error| match error {
                std::env::VarError::NotPresent => Ok(None),
                std::env::VarError::NotUnicode(_) => Err(invalid(
                    "SUBWAY_CONGESTION_AUTO_DISCOVERY must be true or false",
                )),
            })?;
        self.auto_discovery = discovery::enabled(value.as_deref())?;
        Ok(self)
    }
    pub async fn congestion(&self, query: &CongestionQuery) -> Result<SubwayCongestionReport> {
        query.validate()?;
        let (mut url, dataset_as_of) = if self.auto_discovery {
            let catalog = reqwest::Url::parse(discovery::CATALOG).map_err(|_| Error::Provider)?;
            let bytes = self.http.get(catalog, "catalog".into()).await?;
            discovery::select(&bytes, chrono::Utc::now().date_naive())?
        } else {
            (self.endpoint.clone(), DATASET_AS_OF.to_string())
        };
        url.query_pairs_mut()
            .append_pair("serviceKey", &self.key)
            .append_pair("page", "1")
            .append_pair("perPage", "2000")
            .append_pair("returnType", "JSON");
        let bytes = self.http.get(url, dataset_as_of.clone()).await?;
        let mut report = wire::parse(&bytes, query)?;
        report.dataset_as_of = dataset_as_of;
        Ok(report)
    }
}
#[cfg(test)]
mod tests;
