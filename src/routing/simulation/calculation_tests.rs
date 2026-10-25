use super::*;
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
fn context() -> TravelContext {
    TravelContext {
        base_trip_minutes: 40,
        station_walk_minutes: 6,
        remaining_train_stops: 10,
        walking_factor: 1.0,
        policy: PlanningPolicy::default(),
        target_arrival: None,
        buffer_minutes: 0,
    }
}
#[test]
fn walking_and_train_buffers_are_separate_assumptions() -> TestResult {
    let result = context().calculate(150.0)?;
    assert_eq!(result.baseline_walk_seconds, 360);
    assert_eq!(result.walking_buffer_seconds, 45);
    assert_eq!(result.walk_seconds_with_buffer, 405);
    assert_eq!(result.train_dwell_buffer_seconds, 50);
    assert_eq!(result.trip_seconds_with_buffer, 2495);
    assert_eq!(result.total_buffer_seconds, 95);
    assert!(!result.validated_prediction);
    assert_eq!(result.basis, "unvalidated_planning_policy_v1");
    Ok(())
}
#[test]
fn no_buffer_below_threshold_and_no_train_buffer_without_stops() -> TestResult {
    let result = context().calculate(92.2)?;
    assert_eq!(result.total_buffer_seconds, 0);
    let mut input = context();
    input.remaining_train_stops = 0;
    assert_eq!(input.calculate(200.0)?.train_dwell_buffer_seconds, 0);
    input.station_walk_minutes = 0;
    assert_eq!(input.calculate(200.0)?.walking_buffer_seconds, 0);
    Ok(())
}
#[test]
fn walking_factor_applies_once_and_departure_includes_buffer() -> TestResult {
    let mut input = context();
    input.walking_factor = 2.0;
    input.target_arrival = Some("2026-10-26T09:00:00+09:00".parse()?);
    input.buffer_minutes = 5;
    let result = input.calculate(150.0)?;
    assert_eq!(result.baseline_trip_seconds, 2760);
    assert_eq!(result.walking_buffer_seconds, 90);
    assert_eq!(result.train_dwell_buffer_seconds, 50);
    assert_eq!(
        result.leave_at_with_buffer.as_deref(),
        Some("2026-10-26T08:06:40+09:00")
    );
    Ok(())
}
#[test]
fn rejects_bad_context_and_supports_explicit_policy() -> TestResult {
    let mut input = context();
    input.station_walk_minutes = 41;
    assert!(input.calculate(150.0).is_err());
    input = context();
    input.walking_factor = f64::NAN;
    assert!(input.validate().is_err());
    input = context();
    input.policy.extra_walk_ratio_per_100_percentage_points = 0.0;
    input
        .policy
        .extra_dwell_seconds_per_stop_per_100_percentage_points = 0.0;
    assert_eq!(input.calculate(200.0)?.total_buffer_seconds, 0);
    assert!(context().calculate(f64::INFINITY).is_err());
    Ok(())
}
