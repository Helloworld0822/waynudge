use axum::http::request::Parts;
use rmcp::{
    ErrorData, Json, RoleServer, ServerHandler,
    handler::server::{tool::schema_for_type, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    service::RequestContext,
    tool, tool_handler, tool_router,
};

use crate::{
    api::User,
    error::Error,
    routing::Comparison,
    service::{
        Brief, BriefRequest, MutationResult, ReplanRequest, RoutineList, Service, UpsertRequest,
    },
    storage::{DayStatus, Feedback},
};

#[derive(Clone)]
pub struct Mcp {
    service: Service,
}

#[tool_router]
impl Mcp {
    pub fn new(service: Service) -> Self {
        Self { service }
    }

    #[tool(description = "List the authenticated user's commute routines.",
        output_schema = schema_for_type::<RoutineList>(),
        annotations(read_only_hint = true))]
    async fn list_routines(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<RoutineList>, ErrorData> {
        self.service
            .list(user(&context)?)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description="Prepare a validated recurring commute reminder schedule and prompt for the host agent scheduler. Does not create an automation.",output_schema=schema_for_type::<crate::reminders::ReminderPlan>(),annotations(read_only_hint=true))]
    async fn prepare_commute_reminder(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<crate::reminders::ReminderRequest>,
    ) -> Result<Json<crate::reminders::ReminderPlan>, ErrorData> {
        self.service
            .prepare_reminder(user(&context)?, input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description="Get real-time TAGO or Gyeonggi bus arrival estimates using provider-specific station IDs. TAGO also requires city_code.",output_schema=schema_for_type::<crate::providers::bus::BusArrivals>(),annotations(read_only_hint=true))]
    async fn get_bus_arrivals(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<crate::providers::bus::BusQuery>,
    ) -> Result<Json<crate::providers::bus::BusArrivals>, ErrorData> {
        user(&context)?;
        self.service
            .provider
            .bus_arrivals(&input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description="Get Seoul subway arrival observations for a station, optionally filtered by line ID and direction. These are not whole-journey delay estimates.",output_schema=schema_for_type::<crate::providers::seoul::SubwayArrivals>(),annotations(read_only_hint=true))]
    async fn get_subway_arrivals(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<crate::providers::seoul::SubwayQuery>,
    ) -> Result<Json<crate::providers::seoul::SubwayArrivals>, ErrorData> {
        user(&context)?;
        self.service
            .provider
            .subway_arrivals(&input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description="Get Seoul Metro lines 1..8 historical 30-minute average train congestion, NOT real-time occupancy. Provide station, line, day_type (weekday/saturday/sunday), time (HH:MM Asia/Seoul), and optional direction (up/down). Always report dataset_as_of and realtime=false.",output_schema=schema_for_type::<crate::providers::congestion::SubwayCongestionReport>(),annotations(read_only_hint=true))]
    async fn get_subway_congestion(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<crate::providers::congestion::CongestionQuery>,
    ) -> Result<Json<crate::providers::congestion::SubwayCongestionReport>, ErrorData> {
        user(&context)?;
        self.service
            .provider
            .subway_congestion(&input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description = "Create or update a commute routine after explicit user confirmation.",
        output_schema = schema_for_type::<MutationResult>(),
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true))]
    async fn upsert_routine(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<UpsertRequest>,
    ) -> Result<Json<MutationResult>, ErrorData> {
        self.service
            .upsert(user(&context)?, input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description = "Get a departure decision and route summary for a commute routine.",
        output_schema = schema_for_type::<Brief>(),
        annotations(read_only_hint = true))]
    async fn get_commute_brief(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<BriefRequest>,
    ) -> Result<Json<Brief>, ErrorData> {
        self.service
            .brief(user(&context)?, input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description = "Compare available transit routes for a commute routine.",
        output_schema = schema_for_type::<Comparison>(),
        annotations(read_only_hint = true))]
    async fn compare_routes(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<BriefRequest>,
    ) -> Result<Json<Comparison>, ErrorData> {
        self.service
            .compare(user(&context)?, input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description = "Replan a commute from its current trip context.",
        output_schema = schema_for_type::<Brief>(),
        annotations(read_only_hint = true))]
    async fn replan_trip(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<ReplanRequest>,
    ) -> Result<Json<Brief>, ErrorData> {
        self.service
            .replan(user(&context)?, input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description = "Enable or skip a routine on a date after explicit user confirmation.",
        output_schema = schema_for_type::<MutationResult>(),
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true))]
    async fn set_routine_day_status(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<DayStatus>,
    ) -> Result<Json<MutationResult>, ErrorData> {
        self.service
            .set_day(user(&context)?, input)
            .await
            .map(Json)
            .map_err(tool_error)
    }

    #[tool(description = "Record observed trip duration and walking feedback after explicit user confirmation.",
        output_schema = schema_for_type::<MutationResult>(),
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false))]
    async fn record_trip_feedback(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(input): Parameters<Feedback>,
    ) -> Result<Json<MutationResult>, ErrorData> {
        self.service
            .feedback(user(&context)?, input)
            .await
            .map(Json)
            .map_err(tool_error)
    }
}

#[tool_handler]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}

fn user(context: &RequestContext<RoleServer>) -> Result<&str, ErrorData> {
    context
        .extensions
        .get::<Parts>()
        .and_then(|parts| parts.extensions.get::<User>())
        .map(|user| user.0.as_str())
        .ok_or_else(|| tool_error(Error::Unauthorized))
}

fn tool_error(error: Error) -> ErrorData {
    match error {
        Error::Invalid(message) => ErrorData::invalid_params(message, None),
        Error::Unauthorized => ErrorData::invalid_request("authentication required", None),
        Error::NotFound => ErrorData::invalid_params("routine not found", None),
        Error::Confirmation => {
            ErrorData::invalid_params("explicit user confirmation required", None)
        }
        Error::Provider => ErrorData::internal_error(
            "transit provider unavailable; no live estimate available",
            None,
        ),
        Error::Storage(_) | Error::Json(_) => {
            tracing::error!("storage operation failed");
            ErrorData::internal_error("storage unavailable", None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_schemas_when_tools_are_listed() {
        // Given: the registered MCP tools.
        let router = Mcp::tool_router();
        // When: a client discovers their schemas.
        let tools = router.list_all();
        // Then: each tool declares structured output and no caller-supplied identity.
        assert_eq!(tools.len(), 11);
        for tool in tools {
            assert!(tool.output_schema.is_some(), "{}", tool.name);
            assert!(
                tool.input_schema
                    .get("properties")
                    .and_then(|properties| properties.get("user_id"))
                    .is_none(),
                "{}",
                tool.name
            );
        }
    }

    #[test]
    fn storage_details_are_redacted_when_operation_fails() {
        // Given: an internal storage failure containing sensitive data.
        let error = Error::Storage(sqlx::Error::Protocol("private database details".into()));
        // When: the boundary converts it to an MCP error.
        let response = tool_error(error);
        // Then: the response contains only the public storage error.
        assert_eq!(response.message, "storage unavailable");
        assert!(response.data.is_none());
    }
}
