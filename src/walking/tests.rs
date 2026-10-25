use super::*;
use crate::{
    error::{Error, PUBLIC_USER},
    providers::Provider,
    routines::Routine,
    storage::Store,
};

#[path = "storage_tests.rs"]
mod persistence;

fn routine() -> Result<Routine> {
    Ok(serde_json::from_value(serde_json::json!({
        "id":"commute", "name":"Work", "origin":{"name":"A","lat":37.5,"lon":127.0},
        "destination":{"name":"B","lat":37.6,"lon":127.1}, "arrival_time":"09:00",
        "weekdays":[1], "preferred_modes":["walk"], "max_transfers":0,
        "max_walk_minutes":60, "buffer_minutes":5, "preparation_minutes":10,
        "min_alert_interval_minutes":10, "walking_factor":2.0, "usual_route_id":null
    }))?)
}

fn observation() -> Result<WalkingObservation> {
    let routine = routine()?;
    Ok(WalkingObservation {
        routine_id: routine.id,
        observation_id: "walk-1".into(),
        origin: routine.origin,
        destination: routine.destination,
        distance_meters: 600.0,
        elapsed_minutes: 10.0,
        distance_source: WalkingDistanceSource::UserMeasured,
        confirmed: true,
    })
}

async fn service() -> Result<Service> {
    let store = Store::open("sqlite::memory:").await?;
    store.upsert("alice", &routine()?).await?;
    Ok(Service {
        store,
        provider: Provider::mock(),
    })
}

#[test]
fn legacy_routine_deserializes_without_speed() -> Result<()> {
    let routine = routine()?;
    assert!(routine.walking_speed_kmh.is_none());
    Ok(())
}

#[test]
fn measured_duration_uses_distance_without_double_factor() -> Result<()> {
    let mut routine = routine()?;
    routine.walking_speed_kmh = Some(3.6);
    let seconds = routine.walking_duration_seconds(Some(600.0), 8)?;
    assert!((seconds - 600.0).abs() < 0.001);
    Ok(())
}

#[test]
fn duration_falls_back_when_distance_or_speed_is_missing() -> Result<()> {
    for (speed, distance) in [
        (Some(3.6), None),
        (Some(3.6), Some(0.0)),
        (None, Some(600.0)),
        (None, None),
    ] {
        let mut routine = routine()?;
        routine.walking_speed_kmh = speed;
        let seconds = routine.walking_duration_seconds(distance, 8)?;
        assert!((seconds - 960.0).abs() < 0.001);
    }
    Ok(())
}

#[test]
fn invalid_distance_and_profile_are_rejected() -> Result<()> {
    for distance in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            routine()?
                .walking_duration_seconds(Some(distance), 8)
                .is_err()
        );
    }
    for speed in [0.0, 0.49, 12.01, f64::NAN, f64::INFINITY] {
        let mut routine = routine()?;
        routine.walking_speed_kmh = Some(speed);
        assert!(routine.validate().is_err());
    }
    Ok(())
}

#[test]
fn invalid_observations_are_rejected() -> Result<()> {
    for (distance, minutes) in [
        (0.0, 10.0),
        (-1.0, 10.0),
        (f64::NAN, 10.0),
        (100_001.0, 1000.0),
        (600.0, 0.0),
        (600.0, f64::INFINITY),
        (600.0, 1441.0),
        (10.0, 10.0),
        (600.0, 1.0),
    ] {
        let mut input = observation()?;
        input.distance_meters = distance;
        input.elapsed_minutes = minutes;
        assert!(input.measured_speed().is_err());
    }
    let mut input = observation()?;
    input.origin.lat = 91.0;
    assert!(input.measured_speed().is_err());
    let mut input = observation()?;
    input.observation_id = "\n".into();
    assert!(input.measured_speed().is_err());
    Ok(())
}

#[tokio::test]
async fn unconfirmed_preview_works_without_reading_closed_storage() -> Result<()> {
    let service = service().await?;
    service.store.pool.close().await;
    let mut input = observation()?;
    input.routine_id = "missing".into();
    input.confirmed = false;
    let preview = service
        .record_walking_observation(PUBLIC_USER, input)
        .await?;
    assert!(!preview.saved);
    assert!((preview.measured_speed_kmh - 3.6).abs() < 0.001);
    assert!(preview.profile_speed_kmh.is_none());
    assert!(preview.sample_count.is_none());
    Ok(())
}

#[tokio::test]
async fn confirmed_observation_requires_private_owner() -> Result<()> {
    let service = service().await?;
    assert!(matches!(
        service
            .record_walking_observation(PUBLIC_USER, observation()?)
            .await,
        Err(Error::Unauthorized)
    ));
    assert!(matches!(
        service
            .record_walking_observation("bob", observation()?)
            .await,
        Err(Error::NotFound)
    ));
    Ok(())
}
