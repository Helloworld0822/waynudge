use super::*;
impl Provider {
    pub(super) fn mock_routes(&self, origin: &Place, destination: &Place) -> RouteSet {
        RouteSet {
            source: "mock".into(),
            warnings: vec![
                "Synthetic demo routes; walking distances assume mock-only 4 km/h, not real transit or live information.".into(),
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
                walk_distance_meters: Some(f64::from(walk_minutes) * 4000.0 / 60.0),
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
                        access_walk_minutes: walk_minutes,
                        access_walk_distance_meters: Some(f64::from(walk_minutes) * 4000.0 / 60.0),
                        station_name: "Mock subway station".into(),
                        line_id: Some("1001".into()),
                        direction: Some(seoul::Direction::Up),
                        live: None,
                    }),
                    Mode::Bus | Mode::Walk => None,
                },
            })
            .collect(),
        }
    }
}
