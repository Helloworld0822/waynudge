use super::*;
impl Provider {
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
