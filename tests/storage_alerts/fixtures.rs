use chrono::{DateTime, Utc};
use waynudge::{
    alerts::AlertDraft,
    routines::{Mode, Place, Routine},
};

pub(super) fn routine() -> Routine {
    Routine {
        id: "commute".into(),
        name: "Commute".into(),
        origin: Place {
            name: "Home".into(),
            lat: 37.5,
            lon: 127.0,
        },
        destination: Place {
            name: "Office".into(),
            lat: 37.6,
            lon: 127.1,
        },
        arrival_time: "09:00".into(),
        weekdays: vec![1, 2, 3, 4, 5],
        preferred_modes: vec![Mode::Subway],
        max_transfers: 2,
        max_walk_minutes: 20,
        buffer_minutes: 5,
        preparation_minutes: 10,
        min_alert_interval_minutes: 10,
        walking_factor: 1.0,
        walking_speed_kmh: None,
        usual_route_id: None,
    }
}

pub(super) fn draft(kind: &str, now: DateTime<Utc>) -> AlertDraft {
    AlertDraft {
        routine_id: "commute".into(),
        date: now.date_naive().to_string(),
        kind: kind.into(),
        now,
        message: "Leave soon".into(),
        min_interval_minutes: 10,
    }
}
