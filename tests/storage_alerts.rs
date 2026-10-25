use chrono::{DateTime, Duration, Utc};
use waynudge::{
    error::Error,
    storage::{DayStatus, Feedback, Store},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn routines_are_isolated_when_ids_overlap() -> TestResult {
    // Given
    let store = Store::open("sqlite::memory:").await?;
    store.upsert("alice", &routine()).await?;
    let mut other = routine();
    other.name = "Bob commute".into();
    // When
    store.upsert("bob", &other).await?;
    // Then
    assert_eq!(store.list("alice").await?[0].name, "Commute");
    assert_eq!(store.get("bob", "commute").await?.name, "Bob commute");
    assert!(matches!(
        store.get("eve", "commute").await,
        Err(Error::NotFound)
    ));
    assert_eq!(store.all_routines().await?.len(), 2);
    store.health().await?;
    Ok(())
}

#[tokio::test]
async fn holiday_is_excluded_only_for_owner_and_date() -> TestResult {
    // Given
    let store = Store::open("sqlite::memory:").await?;
    store.upsert("alice", &routine()).await?;
    store.upsert("bob", &routine()).await?;
    let input = DayStatus {
        routine_id: "commute".into(),
        date: "2026-10-26".into(),
        enabled: false,
        confirmed: true,
    };
    // When
    store.set_day("alice", &input).await?;
    // Then
    assert_eq!(
        store.day_enabled("alice", "commute", "2026-10-26").await?,
        Some(false)
    );
    assert_eq!(
        store.day_enabled("alice", "commute", "2026-10-27").await?,
        None
    );
    assert_eq!(
        store.day_enabled("bob", "commute", "2026-10-26").await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn mutations_reject_unconfirmed_invalid_and_foreign_input() -> TestResult {
    // Given
    let store = Store::open("sqlite::memory:").await?;
    store.upsert("alice", &routine()).await?;
    let mut day = DayStatus {
        routine_id: "commute".into(),
        date: "2026-10-26".into(),
        enabled: false,
        confirmed: false,
    };
    let mut input = Feedback {
        routine_id: "commute".into(),
        route_id: "route".into(),
        actual_minutes: 35,
        walking_factor: Some(1.5),
        confirmed: false,
    };
    // When / Then
    assert!(matches!(
        store.set_day("alice", &day).await,
        Err(Error::Confirmation)
    ));
    assert!(matches!(
        store.feedback("alice", &input).await,
        Err(Error::Confirmation)
    ));
    day.confirmed = true;
    input.confirmed = true;
    assert!(matches!(
        store.set_day("eve", &day).await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        store.feedback("eve", &input).await,
        Err(Error::NotFound)
    ));
    day.date = "2026-02-30".into();
    assert!(matches!(
        store.set_day("alice", &day).await,
        Err(Error::Invalid(_))
    ));
    for factor in [f64::NAN, 0.4, 3.1] {
        input.walking_factor = Some(factor);
        assert!(matches!(
            store.feedback("alice", &input).await,
            Err(Error::Invalid(_))
        ));
    }
    input.walking_factor = None;
    input.actual_minutes = 0;
    assert!(matches!(
        store.feedback("alice", &input).await,
        Err(Error::Invalid(_))
    ));
    assert_eq!(store.get("alice", "commute").await?.walking_factor, 1.0);
    Ok(())
}

#[tokio::test]
async fn feedback_updates_only_owners_walking_factor() -> TestResult {
    // Given
    let store = Store::open("sqlite::memory:").await?;
    store.upsert("alice", &routine()).await?;
    store.upsert("bob", &routine()).await?;
    let input = Feedback {
        routine_id: "commute".into(),
        route_id: "route".into(),
        actual_minutes: 35,
        walking_factor: Some(1.5),
        confirmed: true,
    };
    // When
    store.feedback("alice", &input).await?;
    // Then
    assert!((store.get("alice", "commute").await?.walking_factor - 1.5).abs() < 0.001);
    assert!((store.get("bob", "commute").await?.walking_factor - 1.0).abs() < 0.001);
    Ok(())
}

#[tokio::test]
async fn alerts_deduplicate_and_space_across_midnight_and_kinds() -> TestResult {
    // Given
    let store = Store::open("sqlite::memory:").await?;
    store.upsert("alice", &routine()).await?;
    store.upsert("bob", &routine()).await?;
    let now: DateTime<Utc> = "2026-10-26T23:59:00Z".parse()?;
    assert!(store.enqueue_alert("alice", &draft("prepare", now)).await?);
    // When / Then
    assert!(
        !store
            .enqueue_alert("alice", &draft("prepare", now + Duration::minutes(1)))
            .await?
    );
    assert!(
        !store
            .enqueue_alert("alice", &draft("leave", now + Duration::minutes(9)))
            .await?
    );
    assert!(
        store
            .enqueue_alert("alice", &draft("leave", now + Duration::minutes(10)))
            .await?
    );
    assert!(
        !store
            .enqueue_alert("alice", &draft("leave", now + Duration::minutes(20)))
            .await?
    );
    assert!(store.enqueue_alert("bob", &draft("prepare", now)).await?);
    assert_eq!(store.alerts("alice").await?.len(), 2);
    assert_eq!(store.alerts("bob").await?.len(), 1);
    assert!(store.alerts("eve").await?.is_empty());
    assert!(matches!(
        store.enqueue_alert("eve", &draft("prepare", now)).await,
        Err(Error::NotFound)
    ));
    Ok(())
}

#[tokio::test]
async fn concurrent_connections_emit_once_and_survive_reopening() -> TestResult {
    // Given: an isolated file and independent connection pools.
    let root = std::env::temp_dir().join(format!(
        "waynudge-store-{}-{}",
        std::process::id(),
        Utc::now()
            .timestamp_nanos_opt()
            .ok_or("clock out of range")?
    ));
    tokio::fs::create_dir(&root).await?;
    let path = root.join("test.db");
    let url = format!("sqlite://{}", path.display());
    let first = Store::open(&url).await?;
    let second = Store::open(&url).await?;
    first.upsert("alice", &routine()).await?;
    first
        .set_day(
            "alice",
            &DayStatus {
                routine_id: "commute".into(),
                date: "2026-10-26".into(),
                enabled: false,
                confirmed: true,
            },
        )
        .await?;
    first
        .feedback(
            "alice",
            &Feedback {
                routine_id: "commute".into(),
                route_id: "route".into(),
                actual_minutes: 40,
                walking_factor: Some(1.8),
                confirmed: true,
            },
        )
        .await?;
    let now: DateTime<Utc> = "2026-10-26T08:00:00Z".parse()?;
    let a = draft("prepare", now);
    let b = draft("leave", now);
    // When
    let (a, b) = tokio::join!(
        first.enqueue_alert("alice", &a),
        second.enqueue_alert("alice", &b)
    );
    assert_ne!(a?, b?);
    drop(first);
    drop(second);
    let reopened = Store::open(&url).await?;
    // Then
    assert_eq!(reopened.alerts("alice").await?.len(), 1);
    assert!((reopened.get("alice", "commute").await?.walking_factor - 1.8).abs() < 0.001);
    assert_eq!(
        reopened
            .day_enabled("alice", "commute", "2026-10-26")
            .await?,
        Some(false)
    );
    let connection = sqlx::SqlitePool::connect(&url).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback WHERE user_id = 'alice'")
        .fetch_one(&connection)
        .await?;
    assert_eq!(count, 1);
    connection.close().await;
    drop(reopened);
    tokio::fs::remove_dir_all(root).await?;
    Ok(())
}

#[path = "storage_alerts/fixtures.rs"]
mod fixtures;
use fixtures::{draft, routine};
