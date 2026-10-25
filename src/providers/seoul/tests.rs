use super::*;
use serde_json::json;
fn query() -> SubwayQuery {
    SubwayQuery {
        station_name: "서울역".into(),
        line_id: Some("1001".into()),
        direction: Some(Direction::Up),
    }
}
fn row(direction: &str, at: &str, seconds: &str, code: &str) -> serde_json::Value {
    json!({"subwayId":"1001","updnLine":direction,"statnNm":"서울","bstatnNm":"의정부","btrainNo":"0084","barvlDt":seconds,"recptnDt":at,"arvlMsg2":"도착정보","arvlCd":code})
}
#[test]
fn filters_direction_departed_and_marks_stale() -> crate::error::Result<()> {
    // Given
    let payload = json!({"errorMessage":{"code":"INFO-000"},"realtimeArrivalList":[row("상행","2026-10-26 08:00:00","60","99"),row("하행","2026-10-26 08:00:00","10","99"),row("상행","2026-10-26 08:00:00","0","2"),row("상행","2026-10-26 07:50:00","180","99")]});
    // When
    let result = parse(
        serde_json::from_value(payload)?,
        &query(),
        "2026-10-25T23:00:30Z"
            .parse()
            .map_err(|_| Error::Provider)?,
    )?;
    // Then
    assert_eq!(result.arrivals.len(), 2);
    assert!(result.arrivals[0].fresh);
    assert!(!result.arrivals[1].fresh);
    assert_eq!(result.arrivals[0].arrival_seconds, Some(60));
    Ok(())
}
#[test]
fn zero_without_arrival_evidence_is_unknown() -> crate::error::Result<()> {
    // Given
    let payload = json!({"realtimeArrivalList":[row("상행","2026-10-26 08:00:00","0","99")]});
    // When
    let result = parse(
        serde_json::from_value(payload)?,
        &query(),
        "2026-10-25T23:00:30Z"
            .parse()
            .map_err(|_| Error::Provider)?,
    )?;
    // Then
    assert_eq!(result.arrivals[0].arrival_seconds, None);
    Ok(())
}
#[test]
fn authentication_errors_never_become_mock() -> crate::error::Result<()> {
    // Given
    let payload = json!({"errorMessage":{"code":"INFO-100"}});
    // When
    let result = parse(serde_json::from_value(payload)?, &query(), Utc::now());
    // Then
    assert!(matches!(result, Err(Error::Provider)));
    assert!(
        SubwayQuery {
            station_name: "../key".into(),
            line_id: None,
            direction: None
        }
        .validate()
        .is_err()
    );
    Ok(())
}
