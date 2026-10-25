use super::*;
use serde_json::json;
#[test]
fn copied_encoded_key_is_trimmed_and_decoded_once() -> Result<()> {
    // Given / When
    let client = BusProvider::new(BusKind::Tago, " test%2Bkey%2F%3D \n".into(), 10)?;
    // Then
    assert_eq!(client.key, "test+key/=");
    Ok(())
}
fn query(kind: BusKind) -> BusQuery {
    BusQuery {
        provider: kind,
        station_id: match kind {
            BusKind::Tago => "DJB8001793".into(),
            BusKind::Gyeonggi => "200000078".into(),
        },
        city_code: Some(25),
        route_id: None,
    }
}
#[test]
fn tago_supports_singleton_and_filters_route() -> Result<()> {
    // Given
    let payload = json!({"response":{"header":{"resultCode":"00"},"body":{"items":{"item":{"routeid":"DJB30300002","routeno":"5","arrtime":816,"arrprevstationcnt":15}}}}});
    // When
    let result = wire::parse(&serde_json::to_vec(&payload)?, &query(BusKind::Tago))?;
    // Then
    assert_eq!(result.arrivals[0].arrival_seconds, 816);
    assert_eq!(result.source, "tago");
    Ok(())
}
#[test]
fn gyeonggi_converts_minutes_and_omits_unknowns() -> Result<()> {
    // Given
    let payload = json!({"response":{"msgHeader":{"resultCode":0},"msgBody":{"busArrivalList":[{"routeId":234000016,"predictTime1":3,"predictTime2":0,"locationNo1":2,"remainSeatCnt1":6,"crowded1":3},{"routeId":234000017,"predictTime1":-1}]}}});
    // When
    let result = wire::parse(&serde_json::to_vec(&payload)?, &query(BusKind::Gyeonggi))?;
    // Then
    assert_eq!(result.arrivals.len(), 1);
    assert_eq!(result.arrivals[0].arrival_seconds, 180);
    assert_eq!(result.arrivals[0].route_id, "234000016");
    assert_eq!(result.arrivals[0].remaining_seats, Some(6));
    assert!(matches!(
        result.arrivals[0].crowding,
        Some(BusCrowding::Crowded)
    ));
    Ok(())
}
#[test]
fn upstream_error_and_missing_city_are_rejected() -> Result<()> {
    // Given
    let payload = json!({"response":{"header":{"resultCode":"30"},"body":null}});
    let mut q = query(BusKind::Tago);
    q.city_code = None;
    // When / Then
    assert!(matches!(
        wire::parse(&serde_json::to_vec(&payload)?, &q),
        Err(Error::Provider)
    ));
    assert!(q.validate().is_err());
    Ok(())
}
