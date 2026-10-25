use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[derive(Deserialize)]
pub(super) struct Payload {
    result: Option<Paths>,
}
#[derive(Deserialize)]
struct Paths {
    path: Vec<Path>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Path {
    info: Info,
    sub_path: Vec<Leg>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    total_time: u32,
    payment: u32,
    total_walk: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Leg {
    traffic_type: u32,
    section_time: u32,
    distance: f64,
    #[serde(default)]
    start_name: String,
    #[serde(default)]
    end_name: String,
    #[serde(default)]
    lane: Vec<Lane>,
    #[serde(default)]
    way_code: Option<u32>,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Lane {
    #[serde(default)]
    name: String,
    #[serde(default)]
    bus_no: String,
    #[serde(default)]
    bus_id: Option<u64>,
    #[serde(default)]
    subway_code: Option<u32>,
}
pub(super) fn parse(payload: Payload, origin: &Place, destination: &Place) -> Result<RouteSet> {
    let paths = payload.result.ok_or(Error::Provider)?.path;
    if paths.is_empty() || paths.len() > 100 {
        return Err(Error::Provider);
    }
    let mut routes = Vec::new();
    for path in paths {
        let mut boarding = None;
        let mut elapsed = 0_u32;
        let (mut subway, mut bus, mut walk, mut legs) = (false, false, 0_u32, 0_u32);
        let mut identity = Sha256::new();
        if path.sub_path.is_empty()
            || path.info.total_time == 0
            || path.info.total_time > 1440
            || path.info.total_walk > 100_000
        {
            return Err(Error::Provider);
        }
        for leg in path.sub_path {
            if leg.section_time > path.info.total_time {
                return Err(Error::Provider);
            }
            if !leg.distance.is_finite() || leg.distance < 0.0 {
                return Err(Error::Provider);
            }
            match leg.traffic_type {
                1 => {
                    if boarding.is_none() {
                        let line_id = leg
                            .lane
                            .first()
                            .and_then(|lane| lane.subway_code)
                            .filter(|code| (1..=9).contains(code))
                            .map(|code| format!("{}", 1000 + code));
                        let direction = match leg.way_code {
                            Some(1) => Some(seoul::Direction::Up),
                            Some(2) => Some(seoul::Direction::Down),
                            Some(_) | None => None,
                        };
                        boarding = Some(seoul::SubwayBoarding {
                            access_minutes: elapsed,
                            access_walk_minutes: walk,
                            station_name: leg.start_name.clone(),
                            line_id,
                            direction,
                            live: None,
                        });
                    }
                    subway = true;
                    legs += 1;
                }
                2 => {
                    bus = true;
                    legs += 1;
                }
                3 => {
                    walk = walk.checked_add(leg.section_time).ok_or(Error::Provider)?;
                }
                _ => return Err(Error::Provider),
            }
            let encoded =
                serde_json::to_vec(&(leg.traffic_type, leg.start_name, leg.end_name, leg.lane))
                    .map_err(|_| Error::Provider)?;
            identity.update(encoded);
            elapsed = elapsed.saturating_add(leg.section_time);
        }
        if walk > path.info.total_time || legs == 0 {
            return Err(Error::Provider);
        }
        routes.push(Route {
            id: format!("odsay-{:x}", identity.finalize()),
            mode: match (subway, bus) {
                (true, true) => Mode::Mixed,
                (true, false) => Mode::Subway,
                (false, true) => Mode::Bus,
                (false, false) => Mode::Walk,
            },
            total_minutes: path.info.total_time,
            walk_minutes: walk,
            transfers: legs.saturating_sub(1),
            cost_won: path.info.payment,
            collected_at: Utc::now(),
            realtime: false,
            delay_minutes: 0,
            origin: origin.clone(),
            destination: destination.clone(),
            subway_boarding: boarding,
        });
    }
    Ok(RouteSet {
        source: "odsay".into(),
        routes,
        warnings: vec![
            "ODsay scheduled route estimates; real-time delay information unavailable.".into(),
        ],
    })
}
