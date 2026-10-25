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
