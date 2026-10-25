mod model;
mod wire;
use super::live_http::LiveHttp;
use crate::error::{Error, Result, invalid};
pub use model::{BusArrival, BusArrivals, BusCrowding, BusKind, BusQuery};
#[derive(Clone)]
pub struct BusProvider {
    kind: BusKind,
    key: String,
    http: LiveHttp,
}
impl BusProvider {
    pub fn new(kind: BusKind, key: String, daily_limit: u32) -> Result<Self> {
        if key.trim().is_empty() || key.len() > 512 || key.bytes().any(|c| !c.is_ascii_graphic()) {
            return Err(invalid(
                "invalid public data service key; use the Decoding key",
            ));
        }
        Ok(Self {
            kind,
            key,
            http: LiveHttp::new(daily_limit)?,
        })
    }
    pub async fn arrivals(&self, query: &BusQuery) -> Result<BusArrivals> {
        query.validate()?;
        let endpoint = match self.kind {
            BusKind::Tago => {
                "https://apis.data.go.kr/1613000/ArvlInfoInqireService/getSttnAcctoArvlPrearngeInfoList"
            }
            BusKind::Gyeonggi => {
                "https://apis.data.go.kr/6410000/busarrivalservice/v2/getBusArrivalListv2"
            }
        };
        let mut url = reqwest::Url::parse(endpoint).map_err(|_| Error::Provider)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("serviceKey", &self.key);
            match self.kind {
                BusKind::Tago => {
                    pairs
                        .append_pair("_type", "json")
                        .append_pair("numOfRows", "100")
                        .append_pair("nodeId", &query.station_id)
                        .append_pair(
                            "cityCode",
                            &query
                                .city_code
                                .ok_or_else(|| invalid("TAGO requires city_code"))?
                                .to_string(),
                        );
                }
                BusKind::Gyeonggi => {
                    pairs
                        .append_pair("format", "json")
                        .append_pair("stationId", &query.station_id);
                }
            }
        }
        let bytes = self
            .http
            .get(
                url,
                format!("{:?}:{}:{:?}", self.kind, query.station_id, query.city_code),
            )
            .await?;
        wire::parse(&bytes, query)
    }
}
#[cfg(test)]
mod tests;
