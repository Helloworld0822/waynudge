use super::{BusArrival, BusArrivals, BusKind, BusQuery};
use crate::error::{Error, Result};
use chrono::Utc;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum Number {
    Integer(i64),
    Text(String),
}
impl Number {
    pub fn text(&self) -> String {
        match self {
            Self::Integer(value) => value.to_string(),
            Self::Text(value) => value.clone(),
        }
    }
    pub fn positive(&self) -> Option<u32> {
        match self {
            Self::Integer(value) => u32::try_from(*value).ok(),
            Self::Text(value) => value.parse().ok(),
        }
    }
}
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Rows<T> {
    Many(Vec<T>),
    One(T),
}
impl<T> Rows<T> {
    fn vec(self) -> Vec<T> {
        match self {
            Self::Many(rows) => rows,
            Self::One(row) => vec![row],
        }
    }
}
#[derive(Deserialize)]
struct TagoRoot {
    response: TagoResponse,
}
#[derive(Deserialize)]
struct TagoResponse {
    header: TagoHeader,
    body: Option<TagoBody>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagoHeader {
    result_code: Number,
}
#[derive(Deserialize)]
struct TagoBody {
    items: Option<TagoItems>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum TagoItems {
    Items { item: Rows<TagoRow> },
    Empty(String),
}
#[derive(Deserialize)]
struct TagoRow {
    routeid: String,
    routeno: Option<Number>,
    arrtime: Number,
    arrprevstationcnt: Option<Number>,
}
#[derive(Deserialize)]
struct GyeonggiRoot {
    response: GyeonggiResponse,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GyeonggiResponse {
    msg_header: GyeonggiHeader,
    msg_body: Option<GyeonggiBody>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GyeonggiHeader {
    result_code: Number,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GyeonggiBody {
    bus_arrival_list: Option<Rows<GyeonggiRow>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GyeonggiRow {
    route_id: Number,
    predict_time1: Option<Number>,
    predict_time2: Option<Number>,
    location_no1: Option<Number>,
    location_no2: Option<Number>,
    remain_seat_cnt1: Option<Number>,
    remain_seat_cnt2: Option<Number>,
    crowded1: Option<Number>,
    crowded2: Option<Number>,
}
pub(super) fn parse(bytes: &[u8], query: &BusQuery) -> Result<BusArrivals> {
    let mut arrivals = Vec::new();
    let source = match query.provider {
        BusKind::Tago => {
            let payload: TagoRoot = serde_json::from_slice(bytes).map_err(|_| Error::Provider)?;
            if !matches!(
                payload.response.header.result_code.text().as_str(),
                "00" | "0"
            ) {
                return Err(Error::Provider);
            }
            let rows = match payload.response.body.and_then(|body| body.items) {
                Some(TagoItems::Items { item }) => item.vec(),
                Some(TagoItems::Empty(value)) if value.is_empty() => vec![],
                None => vec![],
                Some(TagoItems::Empty(_)) => return Err(Error::Provider),
            };
            if rows.len() > 100 {
                return Err(Error::Provider);
            }
            for row in rows {
                if query.route_id.as_ref().is_some_and(|id| id != &row.routeid) {
                    continue;
                }
                let Some(seconds) = row.arrtime.positive().filter(|value| *value <= 86_400) else {
                    continue;
                };
                arrivals.push(BusArrival {
                    remaining_seats: None,
                    crowding_code: None,
                    crowding: None,
                    route_id: row.routeid,
                    route_number: row.routeno.map(|n| n.text()),
                    arrival_seconds: seconds,
                    stops_away: row.arrprevstationcnt.and_then(|n| n.positive()),
                });
            }
            "tago"
        }
        BusKind::Gyeonggi => {
            let payload: GyeonggiRoot =
                serde_json::from_slice(bytes).map_err(|_| Error::Provider)?;
            let code = payload
                .response
                .msg_header
                .result_code
                .positive()
                .ok_or(Error::Provider)?;
            if code != 0 {
                return Err(Error::Provider);
            }
            let rows = payload
                .response
                .msg_body
                .and_then(|body| body.bus_arrival_list)
                .map(Rows::vec)
                .unwrap_or_default();
            if rows.len() > 100 {
                return Err(Error::Provider);
            }
            for row in rows {
                let id = row.route_id.text();
                if query.route_id.as_ref().is_some_and(|wanted| wanted != &id) {
                    continue;
                }
                for (wait, stops, seats, crowding) in [
                    (
                        row.predict_time1,
                        row.location_no1,
                        row.remain_seat_cnt1,
                        row.crowded1,
                    ),
                    (
                        row.predict_time2,
                        row.location_no2,
                        row.remain_seat_cnt2,
                        row.crowded2,
                    ),
                ] {
                    let Some(seconds) = wait
                        .and_then(|n| n.positive())
                        .filter(|n| *n > 0)
                        .and_then(|n| n.checked_mul(60))
                        .filter(|n| *n <= 86_400)
                    else {
                        continue;
                    };
                    arrivals.push(BusArrival {
                        remaining_seats: seats.and_then(|value| value.positive()),
                        crowding_code: crowding
                            .as_ref()
                            .and_then(|value| value.positive())
                            .map(|value| value.to_string()),
                        crowding: crowding
                            .and_then(|value| value.positive())
                            .and_then(super::BusCrowding::from_code),
                        route_id: id.clone(),
                        route_number: None,
                        arrival_seconds: seconds,
                        stops_away: stops.and_then(|n| n.positive()),
                    });
                }
            }
            "gyeonggi_bus"
        }
    };
    arrivals.sort_by(|a, b| {
        a.arrival_seconds
            .cmp(&b.arrival_seconds)
            .then(a.route_id.cmp(&b.route_id))
    });
    let mut warnings=vec!["Provider generation time is unavailable; collected_at is retrieval time only. Arrival estimates are not whole-route delay measurements.".into()];
    if arrivals.is_empty() {
        warnings.push("No matching arrivals; this does not prove a service suspension.".into());
    }
    Ok(BusArrivals {
        source: source.into(),
        station_id: query.station_id.clone(),
        collected_at: Utc::now(),
        arrivals,
        warnings,
    })
}
