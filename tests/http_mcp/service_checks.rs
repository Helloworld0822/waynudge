use super::*;

#[tokio::test]
async fn simulated_delay_changes_route_and_enqueues_disruption_alert() -> TestResult {
    // Given
    let (_, mut service) = app().await?;
    service.provider = Provider::mock_with_delay(25)?;
    let mut routine = routine()?;
    routine.usual_route_id = Some("mock-subway".into());
    service
        .upsert(
            "alice",
            UpsertRequest {
                routine,
                confirmed: true,
            },
        )
        .await?;
    let now = "2026-10-25T23:10:00Z".parse()?;
    // When
    let brief = service
        .brief_at(
            "alice",
            BriefRequest {
                routine_id: "work".into(),
                date: "2026-10-26".into(),
                current_route_id: Some("mock-subway".into()),
            },
            now,
        )
        .await?;
    // Then
    assert!(brief.comparison.should_switch);
    assert_eq!(brief.comparison.source, "mock");
    assert!(
        brief
            .comparison
            .candidates
            .iter()
            .all(|route| !route.realtime)
    );
    assert_eq!(service.evaluate_alerts("alice", now).await?, 1);
    assert_eq!(
        service
            .store
            .alerts("alice")
            .await?
            .first()
            .ok_or("missing alert")?
            .kind,
        "route_change"
    );
    Ok(())
}

#[tokio::test]
async fn departure_and_alerts_follow_exclusions_and_deduplication() -> TestResult {
    // Given
    let (_, service) = app().await?;
    service
        .upsert(
            "alice",
            UpsertRequest {
                routine: routine()?,
                confirmed: true,
            },
        )
        .await?;
    let now = "2026-10-25T23:10:00Z".parse()?;
    // When
    let brief = service
        .brief_at(
            "alice",
            BriefRequest {
                routine_id: "work".into(),
                date: "2026-10-26".into(),
                current_route_id: None,
            },
            now,
        )
        .await?;
    // Then
    assert_eq!(brief.leave_at.as_deref(), Some("2026-10-26T08:20:00+09:00"));
    assert_eq!(
        brief.prepare_at.as_deref(),
        Some("2026-10-26T08:10:00+09:00")
    );
    assert_eq!(service.evaluate_alerts("alice", now).await?, 1);
    assert_eq!(service.evaluate_alerts("alice", now).await?, 0);
    service
        .set_day(
            "alice",
            DayStatus {
                routine_id: "work".into(),
                date: "2026-10-26".into(),
                enabled: false,
                confirmed: true,
            },
        )
        .await?;
    assert_eq!(service.evaluate_alerts("alice", now).await?, 0);
    assert_eq!(service.store.alerts("alice").await?.len(), 1);
    Ok(())
}
#[tokio::test]
async fn missed_transfer_replans_only_from_reported_origin() -> TestResult {
    // Given
    let (app, service) = app().await?;
    service
        .upsert(
            "alice",
            UpsertRequest {
                routine: routine()?,
                confirmed: true,
            },
        )
        .await?;
    let body = json!({"routine_id":"work","date":"2026-10-26","current_route_id":"mock-mixed","current_location":{"name":"Transfer station","lat":37.55,"lon":127.05},"stage":"transfer_missed","blocked_route_ids":["mock-mixed"]});
    // When
    let (status, result) = request(app, "/api/replan", Some(ALICE), Some(body)).await?;
    // Then
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["comparison"]["selected"]["id"], "mock-subway");
    assert_eq!(
        result["comparison"]["selected"]["origin"]["name"],
        "Transfer station"
    );
    assert!(
        result["comparison"]["candidates"]
            .as_array()
            .ok_or("missing candidates")?
            .iter()
            .all(|r| r["id"] != "mock-mixed")
    );
    Ok(())
}

#[tokio::test]
async fn overnight_alert_uses_arrival_date_exclusion() -> TestResult {
    // Given
    let (_, service) = app().await?;
    let mut routine = routine()?;
    routine.arrival_time = "00:30".into();
    service
        .upsert(
            "alice",
            UpsertRequest {
                routine,
                confirmed: true,
            },
        )
        .await?;
    let now = "2026-10-26T14:40:00Z".parse()?;
    // When / Then
    assert_eq!(service.evaluate_alerts("alice", now).await?, 1);
    service
        .set_day(
            "alice",
            DayStatus {
                routine_id: "work".into(),
                date: "2026-10-27".into(),
                enabled: false,
                confirmed: true,
            },
        )
        .await?;
    assert_eq!(service.evaluate_alerts("alice", now).await?, 0);
    Ok(())
}
#[tokio::test]
async fn configured_holiday_suppresses_alert_until_explicit_override() -> TestResult {
    // Given
    let (_, mut service) = app().await?;
    service.holidays.insert("2026-10-26".into());
    service
        .upsert(
            "alice",
            UpsertRequest {
                routine: routine()?,
                confirmed: true,
            },
        )
        .await?;
    let now = "2026-10-25T23:10:00Z".parse()?;
    // When / Then
    assert_eq!(service.evaluate_alerts("alice", now).await?, 0);
    service
        .set_day(
            "alice",
            DayStatus {
                routine_id: "work".into(),
                date: "2026-10-26".into(),
                enabled: true,
                confirmed: true,
            },
        )
        .await?;
    assert_eq!(service.evaluate_alerts("alice", now).await?, 1);
    Ok(())
}
