use chrono::{Duration, Utc};
use waynudge::{
    error::Result,
    providers::Provider,
    routines::{Mode, Place, Routine},
    routing::{CompareInput, compare},
};
async fn input() -> Result<CompareInput> {
    let place = Place {
        name: "test".into(),
        lat: 37.5,
        lon: 127.0,
    };
    let now = Utc::now();
    let mut routes = Provider::mock().routes(&place, &place).await?;
    for route in &mut routes.routes {
        route.realtime = true;
        route.collected_at = now;
    }
    Ok(CompareInput {
        routine: Routine {
            id: "test".into(),
            name: "test".into(),
            origin: place.clone(),
            destination: place,
            arrival_time: "09:00".into(),
            weekdays: vec![1],
            preferred_modes: vec![Mode::Subway, Mode::Bus, Mode::Mixed],
            max_transfers: 3,
            max_walk_minutes: 30,
            buffer_minutes: 0,
            preparation_minutes: 0,
            min_alert_interval_minutes: 30,
            walking_factor: 1.0,
            walking_speed_kmh: None,
            usual_route_id: None,
        },
        routes,
        current_route_id: Some("mock-subway".into()),
        now,
        target: now + Duration::minutes(60),
    })
}
#[tokio::test]
async fn holds_for_one_minute_saving() -> Result<()> {
    let mut input = input().await?;
    input.routes.routes[0].total_minutes = 36;
    let result = compare(&input)?;
    assert!(!result.should_switch);
    assert_eq!(
        result.selected.map(|r| r.id).as_deref(),
        Some("mock-subway")
    );
    Ok(())
}
#[tokio::test]
async fn switches_for_large_fresh_delay() -> Result<()> {
    let mut input = input().await?;
    input.routes.routes[0].delay_minutes = 25;
    let result = compare(&input)?;
    assert!(result.should_switch);
    assert_eq!(result.selected.map(|r| r.id).as_deref(), Some("mock-mixed"));
    Ok(())
}
#[tokio::test]
async fn filters_incumbent_using_adjusted_walking_and_mode() -> Result<()> {
    let mut input = input().await?;
    input.routine.walking_factor = 2.0;
    input.routine.max_walk_minutes = 10;
    input.routine.preferred_modes = vec![Mode::Bus];
    let result = compare(&input)?;
    assert!(result.should_switch);
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.selected.map(|r| (r.mode, r.walk_minutes)),
        Some((Mode::Bus, 8))
    );
    Ok(())
}
#[tokio::test]
async fn warns_and_ignores_stale_delay() -> Result<()> {
    let mut input = input().await?;
    input.routes.warnings.clear();
    input.routes.routes[0].collected_at = input.now - Duration::minutes(6);
    input.routes.routes[0].delay_minutes = 100;
    let result = compare(&input)?;
    assert!(!result.warnings.is_empty());
    assert!(!result.should_switch);
    assert_eq!(result.selected.map(|r| r.delay_minutes), Some(0));
    Ok(())
}
#[tokio::test]
async fn mock_routes_are_explicit_estimates() -> Result<()> {
    let place = Place {
        name: "test".into(),
        lat: 37.5,
        lon: 127.0,
    };
    let routes = Provider::mock().routes(&place, &place).await?;
    assert_eq!(routes.source, "mock");
    assert_eq!(routes.routes.len(), 3);
    assert!(!routes.warnings.is_empty());
    assert!(
        routes
            .routes
            .iter()
            .all(|r| !r.realtime && r.delay_minutes == 0)
    );
    Ok(())
}

#[tokio::test]
async fn measured_speed_adjusts_only_walking_without_applying_factor_twice() -> Result<()> {
    // Given: 30 vehicle minutes, 8 baseline walking minutes, 600 meters and a 7 minute delay.
    let mut input = input().await?;
    input.routine.walking_speed_kmh = Some(6.0);
    input.routine.walking_factor = 2.5;
    input.routes.routes.truncate(1);
    input.routes.routes[0].walk_distance_meters = Some(600.0);
    input.routes.routes[0].delay_minutes = 7;
    // When
    let result = compare(&input)?;
    // Then: 6 walking minutes, 30 unchanged vehicle minutes, unchanged delay.
    let route = result.selected.ok_or(waynudge::error::Error::Provider)?;
    assert_eq!(
        (route.walk_minutes, route.total_minutes, route.delay_minutes),
        (6, 36, 7)
    );
    Ok(())
}

#[tokio::test]
async fn missing_distance_uses_factor_even_with_measured_speed() -> Result<()> {
    // Given
    let mut input = input().await?;
    input.routes.warnings.clear();
    input.routine.walking_speed_kmh = Some(6.0);
    input.routine.walking_factor = 1.5;
    input.routes.routes.truncate(1);
    input.routes.routes[0].walk_distance_meters = None;
    // When
    let result = compare(&input)?;
    // Then
    let route = result.selected.ok_or(waynudge::error::Error::Provider)?;
    assert_eq!((route.walk_minutes, route.total_minutes), (12, 42));
    assert_eq!(result.warnings.len(), 1);
    Ok(())
}

#[tokio::test]
async fn fractional_walking_second_rounds_up_to_the_next_minute() -> Result<()> {
    // Given: 100.01 meters at 6 km/h takes just over one minute.
    let mut input = input().await?;
    input.routine.walking_speed_kmh = Some(6.0);
    input.routes.routes.truncate(1);
    input.routes.routes[0].walk_distance_meters = Some(100.01);
    // When
    let result = compare(&input)?;
    // Then
    let route = result.selected.ok_or(waynudge::error::Error::Provider)?;
    assert_eq!((route.walk_minutes, route.total_minutes), (2, 32));
    Ok(())
}

#[tokio::test]
async fn legacy_route_without_distance_fields_deserializes() -> Result<()> {
    // Given
    let input = input().await?;
    let mut json = serde_json::to_value(&input.routes.routes[0])?;
    json.as_object_mut()
        .ok_or(waynudge::error::Error::Provider)?
        .remove("walk_distance_meters");
    json["subway_boarding"]
        .as_object_mut()
        .ok_or(waynudge::error::Error::Provider)?
        .remove("access_walk_distance_meters");
    // When
    let route: waynudge::routing::Route = serde_json::from_value(json)?;
    // Then
    assert!(route.walk_distance_meters.is_none());
    assert!(
        route
            .subway_boarding
            .ok_or(waynudge::error::Error::Provider)?
            .access_walk_distance_meters
            .is_none()
    );
    Ok(())
}
