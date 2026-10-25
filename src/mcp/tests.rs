use super::*;
#[test]
fn structured_schemas_when_tools_are_listed() {
    let tools = Mcp::tool_router().list_all();
    assert_eq!(tools.len(), 12);
    for tool in tools {
        assert!(tool.output_schema.is_some(), "{}", tool.name);
        assert!(
            tool.input_schema
                .get("properties")
                .and_then(|p| p.get("user_id"))
                .is_none(),
            "{}",
            tool.name
        );
    }
}
#[test]
fn storage_details_are_redacted_when_operation_fails() {
    let error = Error::Storage(sqlx::Error::Protocol("private database details".into()));
    let response = tool_error(error);
    assert_eq!(response.message, "storage unavailable");
    assert!(response.data.is_none());
}
