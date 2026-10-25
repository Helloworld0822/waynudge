use axum::http::request::Parts;
use rmcp::{ErrorData, RoleServer, service::RequestContext};

use crate::{api::User, error::Error};

pub(super) fn user(context: &RequestContext<RoleServer>) -> Result<&str, ErrorData> {
    context
        .extensions
        .get::<Parts>()
        .and_then(|parts| parts.extensions.get::<User>())
        .map(|user| user.0.as_str())
        .ok_or_else(|| tool_error(Error::Unauthorized))
}

pub(super) fn tool_error(error: Error) -> ErrorData {
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
