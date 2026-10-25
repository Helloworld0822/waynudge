use super::*;

#[tokio::test]
async fn observations_weight_time_and_update_only_owner_profile() -> Result<()> {
    let service = service().await?;
    service.store.upsert("bob", &routine()?).await?;
    service
        .record_walking_observation("alice", observation()?)
        .await?;
    let mut second = observation()?;
    second.observation_id = "walk-2".into();
    second.distance_meters = 2400.0;
    second.elapsed_minutes = 20.0;
    second.distance_source = WalkingDistanceSource::MapWalkingRoute;
    let result = service.record_walking_observation("alice", second).await?;
    assert!(result.saved);
    assert_eq!(result.sample_count, Some(2));
    assert!((result.profile_speed_kmh.ok_or(Error::NotFound)? - 6.0).abs() < 0.001);
    let owner = service.store.get("alice", "commute").await?;
    assert!((owner.walking_speed_kmh.ok_or(Error::NotFound)? - 6.0).abs() < 0.001);
    assert!((owner.walking_factor - 2.0).abs() < 0.001);
    assert!(
        service
            .store
            .get("bob", "commute")
            .await?
            .walking_speed_kmh
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn identical_retry_is_idempotent() -> Result<()> {
    let service = service().await?;
    service
        .record_walking_observation("alice", observation()?)
        .await?;
    let result = service
        .record_walking_observation("alice", observation()?)
        .await?;
    assert_eq!(result.sample_count, Some(1));
    assert!((result.profile_speed_kmh.ok_or(Error::NotFound)? - 3.6).abs() < 0.001);
    Ok(())
}

#[tokio::test]
async fn conflicting_retry_is_rejected_without_changing_profile() -> Result<()> {
    let service = service().await?;
    service
        .record_walking_observation("alice", observation()?)
        .await?;
    let mut changed = observation()?;
    changed.distance_meters = 1000.0;
    let result = service.record_walking_observation("alice", changed).await;
    assert!(matches!(result, Err(Error::Invalid(_))));
    let profile = service.store.get("alice", "commute").await?;
    assert!((profile.walking_speed_kmh.ok_or(Error::NotFound)? - 3.6).abs() < 0.001);
    Ok(())
}

#[tokio::test]
async fn legacy_upsert_preserves_calibration_but_explicit_speed_overrides() -> Result<()> {
    let service = service().await?;
    service
        .record_walking_observation("alice", observation()?)
        .await?;
    service.store.upsert("alice", &routine()?).await?;
    let preserved = service.store.get("alice", "commute").await?;
    assert!((preserved.walking_speed_kmh.ok_or(Error::NotFound)? - 3.6).abs() < 0.001);
    let mut replacement = routine()?;
    replacement.walking_speed_kmh = Some(5.0);
    service.store.upsert("alice", &replacement).await?;
    let updated = service.store.get("alice", "commute").await?;
    assert!((updated.walking_speed_kmh.ok_or(Error::NotFound)? - 5.0).abs() < 0.001);
    Ok(())
}

#[tokio::test]
async fn failed_profile_update_rolls_back_observation() -> Result<()> {
    let service = service().await?;
    sqlx::raw_sql("CREATE TRIGGER fail_update BEFORE UPDATE ON routines BEGIN SELECT RAISE(ABORT, 'test update failure'); END;")
        .execute(&service.store.pool).await?;
    let result = service
        .record_walking_observation("alice", observation()?)
        .await;
    assert!(matches!(result, Err(Error::Storage(_))));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM walking_observations")
        .fetch_one(&service.store.pool)
        .await?;
    assert_eq!(count, 0);
    assert!(
        service
            .store
            .get("alice", "commute")
            .await?
            .walking_speed_kmh
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_retries_count_once() -> Result<()> {
    let service = service().await?;
    let (first, second) = tokio::join!(
        service.record_walking_observation("alice", observation()?),
        service.record_walking_observation("alice", observation()?),
    );
    assert_eq!(first?.sample_count, Some(1));
    assert_eq!(second?.sample_count, Some(1));
    Ok(())
}
