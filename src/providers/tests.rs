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
