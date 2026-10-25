use super::*;
use serde_json::json;
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn selects_newest_valid_dataset_and_ignores_future_invalid_or_foreign_paths() -> TestResult {
    let payload = json!({"host":"api.odcloud.kr","basePath":"/api","schemes":["https"],"paths":{
        "/15071311/v1/uddi:ec6e85c3-f01a-415d-8705-e20650f1be84":{"get":{"summary":"서울교통공사_지하철혼잡도정보_20260630"}},
        "/15071311/v1/uddi:00000000-0000-0000-0000-000000000001":{"get":{"summary":"서울교통공사_지하철혼잡도정보_20260930"}},
        "/15071311/v1/uddi:00000000-0000-0000-0000-000000000002":{"get":{"summary":"서울교통공사_지하철혼잡도정보_20261231"}},
        "/15071311/v1/uddi:00000000-0000-0000-0000-000000000003":{"get":{"summary":"서울교통공사_지하철혼잡도정보_20260931"}},
        "/15071311/v1/uddi:../../evil":{"get":{"summary":"서울교통공사_지하철혼잡도정보_20261001"}},
        "https://evil.example/":{"get":{"summary":"서울교통공사_지하철혼잡도정보_20261002"}}
    }});
    let (url, date) = select(
        &serde_json::to_vec(&payload)?,
        NaiveDate::parse_from_str("2026-10-25", "%Y-%m-%d")?,
    )?;
    assert_eq!(date, "2026-09-30");
    assert_eq!(
        url.as_str(),
        "https://api.odcloud.kr/api/15071311/v1/uddi:00000000-0000-0000-0000-000000000001"
    );
    Ok(())
}

#[test]
fn rejects_untrusted_hosts_and_defaults_to_pinned_mode() -> TestResult {
    let payload = json!({"host":"evil.example","basePath":"/api","schemes":["https"],"paths":{}});
    assert!(
        select(
            &serde_json::to_vec(&payload)?,
            chrono::Utc::now().date_naive()
        )
        .is_err()
    );
    assert!(!enabled(None)?);
    assert!(!enabled(Some("false"))?);
    assert!(enabled(Some("true"))?);
    assert!(enabled(Some("TRUE")).is_err());
    Ok(())
}

#[test]
fn rejects_catalogs_without_a_current_trustworthy_dataset() -> TestResult {
    // Given
    let today = NaiveDate::parse_from_str("2026-10-25", "%Y-%m-%d")?;
    for summary in [
        "서울교통공사_지하철혼잡도정보_20260331",
        "서울교통공사_지하철혼잡도정보_20261231",
        "other_20260930",
    ] {
        let payload = json!({"host":"api.odcloud.kr","basePath":"/api","schemes":["https"],"paths":{
            "/15071311/v1/uddi:00000000-0000-0000-0000-000000000001":{"get":{"summary":summary}}
        }});
        // When / Then
        assert!(select(&serde_json::to_vec(&payload)?, today).is_err());
    }
    Ok(())
}
