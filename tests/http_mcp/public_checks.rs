use super::*;
#[tokio::test]
async fn multiple_travel_options_are_comparable_over_public_mcp() -> TestResult {
    // Given: synthetic alternatives and no authenticated user or saved routine.
    let (_, service) = app().await?;
    let auth =
        Auth::new(HashMap::from([("alice".into(), ALICE.into())]), None)?.with_public_mcp(true);
    let app = api::router(service, auth);
    let options: Value = serde_json::from_str(include_str!("../../examples/travel-options.json"))?;
    // When
    let (status, response) = request(
        app,
        "/mcp",
        None,
        Some(json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"compare_travel_options","arguments":options}})),
    )
    .await?;
    // Then: all four modes remain available and bus time is not affected by train congestion.
    assert_eq!(status, StatusCode::OK, "{response}");
    let report = &response["result"]["structuredContent"];
    assert_eq!(report["source"], "simulation", "{response}");
    assert_eq!(report["data_source"], "mock");
    assert_eq!(report["realtime"], false);
    assert_eq!(report["validated_prediction"], false);
    let scenarios = report["scenarios"].as_array().ok_or("missing scenarios")?;
    assert_eq!(scenarios.len(), 3);
    for scenario in scenarios {
        let candidates = scenario["candidates"]
            .as_array()
            .ok_or("missing candidates")?;
        assert_eq!(candidates.len(), 4);
        let bus = candidates
            .iter()
            .find(|c| c["mode"] == "bus")
            .ok_or("missing bus")?;
        assert_eq!(bus["total_seconds"], 1980);
        assert_eq!(bus["train_dwell_buffer_seconds"], 0);
    }
    Ok(())
}

#[tokio::test]
async fn walking_preview_is_public_but_saving_requires_authentication() -> TestResult {
    // Given: anonymous MCP access and synthetic measured distance, with no saved routine.
    let (_, service) = app().await?;
    let auth =
        Auth::new(HashMap::from([("alice".into(), ALICE.into())]), None)?.with_public_mcp(true);
    let app = api::router(service.clone(), auth);
    let observation: Value =
        serde_json::from_str(include_str!("../../examples/walking-observation.json"))?;
    // When / Then: preview calculates speed without accessing a routine; saving is denied.
    for confirmed in [false, true] {
        let mut input = observation.clone();
        input["confirmed"] = json!(confirmed);
        let (status, response) = request(
            app.clone(),
            "/mcp",
            None,
            Some(json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                "params":{"name":"record_walking_observation","arguments":input}})),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{response}");
        if confirmed {
            assert!(response.get("error").is_some() || response["result"]["isError"] == true);
        } else {
            let result = &response["result"]["structuredContent"];
            assert_eq!(result["saved"], false, "{response}");
            assert!(
                (result["measured_speed_kmh"]
                    .as_f64()
                    .ok_or("missing speed")?
                    - 5.0)
                    .abs()
                    < 0.001
            );
            assert!(result["profile_speed_kmh"].is_null());
            assert!(result["sample_count"].is_null());
        }
    }
    assert!(
        service
            .store
            .list(waynudge::error::PUBLIC_USER)
            .await?
            .is_empty()
    );
    assert_eq!(
        request(app, "/api/walking/observations", None, Some(observation))
            .await?
            .0,
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}

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
        14
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
