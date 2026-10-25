use super::*;
fn place() -> Place {
    Place {
        name: "test".into(),
        lat: 37.5,
        lon: 127.0,
    }
}
fn fixture(minutes: u32, traffic: u32) -> String {
    format!(
        r#"{{"result":{{"path":[{{"info":{{"totalTime":{minutes},"payment":1500,"totalWalk":300}},"subPath":[{{"trafficType":3,"sectionTime":5,"distance":300}},{{"trafficType":{traffic},"sectionTime":25,"distance":5000,"startName":"A","endName":"B","lane":[{{"name":"Line 2","subwayCode":2}}]}}]}}]}}}}"#
    )
}
#[test]
fn parses_estimates_with_stable_identity() -> Result<()> {
    let origin = place();
    let a = parse(serde_json::from_str(&fixture(30, 1))?, &origin, &origin)?;
    let b = parse(serde_json::from_str(&fixture(40, 1))?, &origin, &origin)?;
    assert_eq!(a.routes[0].id, b.routes[0].id);
    assert_eq!(a.routes[0].walk_minutes, 5);
    assert_eq!(a.routes[0].transfers, 0);
    assert!(!a.routes[0].realtime);
    assert!(parse(serde_json::from_str(&fixture(30, 4))?, &origin, &origin).is_err());
    Ok(())
}

#[test]
fn separates_subway_access_distance_from_transfer_and_exit_walks() -> Result<()> {
    // Given: bus access, two subway rides and four walking legs.
    let payload = serde_json::json!({"result":{"path":[{
        "info":{"totalTime":55,"payment":1500,"totalWalk":650},
        "subPath":[
            {"trafficType":3,"sectionTime":2,"distance":100},
            {"trafficType":2,"sectionTime":15,"distance":5000},
            {"trafficType":3,"sectionTime":3,"distance":200},
            {"trafficType":1,"sectionTime":20,"distance":6000,"startName":"A"},
            {"trafficType":3,"sectionTime":4,"distance":300},
            {"trafficType":1,"sectionTime":10,"distance":3000,"startName":"B"},
            {"trafficType":3,"sectionTime":1,"distance":50}
        ]
    }]}});
    // When
    let routes = parse(serde_json::from_value(payload)?, &place(), &place())?;
    // Then
    let route = &routes.routes[0];
    assert_eq!(route.walk_minutes, 10);
    assert_eq!(route.transfers, 2);
    assert!(
        route
            .walk_distance_meters
            .is_some_and(|distance| (distance - 650.0).abs() < 0.001)
    );
    let boarding = route.subway_boarding.as_ref().ok_or(Error::Provider)?;
    assert_eq!(
        (boarding.access_minutes, boarding.access_walk_minutes),
        (20, 5)
    );
    assert!(
        boarding
            .access_walk_distance_meters
            .is_some_and(|distance| (distance - 300.0).abs() < 0.001)
    );
    Ok(())
}

#[test]
fn rejects_negative_and_overflowing_walking_distances() -> Result<()> {
    for distances in [[-1.0, 300.0], [f64::MAX, f64::MAX]] {
        // Given
        let payload = serde_json::json!({"result":{"path":[{
            "info":{"totalTime":30,"payment":1500,"totalWalk":600},
            "subPath":[
                {"trafficType":3,"sectionTime":2,"distance":distances[0]},
                {"trafficType":3,"sectionTime":3,"distance":distances[1]},
                {"trafficType":1,"sectionTime":25,"distance":5000}
            ]
        }]}});
        // When
        let result = parse(serde_json::from_value(payload)?, &place(), &place());
        // Then
        assert!(matches!(result, Err(Error::Provider)));
    }
    Ok(())
}

#[test]
fn incomplete_walking_distances_preserve_minutes_for_fallback() -> Result<()> {
    for distance in [serde_json::Value::Null, serde_json::json!(0)] {
        // Given: only one of two access walks has a usable distance.
        let payload = serde_json::json!({"result":{"path":[{
            "info":{"totalTime":30,"payment":1500,"totalWalk":300},
            "subPath":[
                {"trafficType":3,"sectionTime":2,"distance":300},
                {"trafficType":3,"sectionTime":3,"distance":distance},
                {"trafficType":1,"sectionTime":25,"distance":5000}
            ]
        }]}});
        // When
        let routes = parse(serde_json::from_value(payload)?, &place(), &place())?;
        // Then: no partial distance can replace the full five-minute walk.
        let route = &routes.routes[0];
        assert_eq!(route.walk_minutes, 5);
        assert!(route.walk_distance_meters.is_none());
        let boarding = route.subway_boarding.as_ref().ok_or(Error::Provider)?;
        assert_eq!(boarding.access_walk_minutes, 5);
        assert!(boarding.access_walk_distance_meters.is_none());
    }
    Ok(())
}
#[tokio::test]
async fn http_failure_is_private_and_consumes_quota() -> Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| Error::Provider)?;
    let address = listener.local_addr().map_err(|_| Error::Provider)?;
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/",
            axum::routing::get(|| async {
                (
                    axum::http::StatusCode::BAD_GATEWAY,
                    "secret-key coordinates",
                )
            }),
        );
        axum::serve(listener, app).await
    });
    let mut provider = Provider::odsay("secret-key".into(), 1, 0)?;
    if let Some(external) = provider.external.as_mut().and_then(Arc::get_mut) {
        external.endpoint = format!("http://{address}/");
    }
    let result = provider.routes(&place(), &place()).await;
    assert!(matches!(result, Err(Error::Provider)));
    if let Some(external) = &provider.external {
        assert_eq!(external.state.lock().await.used, 1);
    }
    assert!(matches!(
        provider.routes(&place(), &place()).await,
        Err(Error::Provider)
    ));
    server.abort();
    Ok(())
}
