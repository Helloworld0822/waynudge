use super::*;
impl Provider {
    pub fn with_subway_congestion(mut self, key: String, daily_limit: u32) -> Result<Self> {
        self.congestion = Some(congestion::SubwayCongestion::new(key, daily_limit)?);
        Ok(self)
    }
    pub async fn subway_congestion(
        &self,
        query: &congestion::CongestionQuery,
    ) -> Result<congestion::SubwayCongestionReport> {
        query.validate()?;
        self.congestion.as_ref().ok_or_else(|| invalid("SUBWAY_CONGESTION_SERVICE_KEY is not configured; apply for Seoul Metro congestion API at data.go.kr/data/15071311/fileData.do"))?.congestion(query).await
    }
    pub fn with_seoul(mut self, key: String, daily_limit: u32) -> Result<Self> {
        self.seoul = Some(SeoulSubway::new(key, daily_limit)?);
        Ok(self)
    }
    pub fn with_bus(mut self, kind: BusKind, key: String, daily_limit: u32) -> Result<Self> {
        let client = Some(BusProvider::new(kind, key, daily_limit)?);
        match kind {
            BusKind::Tago => self.tago = client,
            BusKind::Gyeonggi => self.gyeonggi = client,
        }
        Ok(self)
    }
    pub async fn bus_arrivals(&self, query: &BusQuery) -> Result<BusArrivals> {
        let client = match query.provider {
            BusKind::Tago => &self.tago,
            BusKind::Gyeonggi => &self.gyeonggi,
        };
        client
            .as_ref()
            .ok_or_else(|| invalid("bus arrival service key is not configured"))?
            .arrivals(query)
            .await
    }
    pub async fn subway_arrivals(&self, query: &SubwayQuery) -> Result<SubwayArrivals> {
        self.seoul
            .as_ref()
            .ok_or_else(|| invalid("SEOUL_SUBWAY_API_KEY is not configured"))?
            .arrivals(query)
            .await
    }
}
