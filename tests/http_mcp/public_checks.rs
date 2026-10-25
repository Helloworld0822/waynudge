use super::*;
#[tokio::test]
async fn public_mcp_connects_without_exposing_or_modifying_personal_routines() -> TestResult {
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
    let auth =
        Auth::new(HashMap::from([("alice".into(), ALICE.into())]), None)?.with_public_mcp(true);
    let app = api::router(service.clone(), auth);
    // When / Then
    let listed = request(
        app.clone(),
        "/mcp",
        None,
        Some(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})),
    )
    .await?;
    assert_eq!(listed.0, StatusCode::OK);
    assert_eq!(
        listed.1["result"]["tools"]
            .as_array()
            .ok_or("missing tools")?
            .len(),
        10
    );
    let listed=request(app.clone(),"/mcp",None,Some(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_routines","arguments":{}}}))).await?;
    assert_eq!(
        listed.1["result"]["structuredContent"]["routines"],
        json!([])
    );
    let mutation=request(app.clone(),"/mcp",None,Some(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"upsert_routine","arguments":{"routine":routine()?,"confirmed":true}}}))).await?;
    assert!(mutation.1.get("error").is_some() || mutation.1["result"]["isError"] == true);
    assert!(
        service
            .store
            .list(waynudge::error::PUBLIC_USER)
            .await?
            .is_empty()
    );
    assert_eq!(service.store.list("alice").await?.len(), 1);
    assert_eq!(
        request(app.clone(), "/health", None, None).await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            app,
            "/mcp",
            Some("invalid"),
            Some(json!({"jsonrpc":"2.0","id":4,"method":"tools/list"}))
        )
        .await?
        .0,
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}
