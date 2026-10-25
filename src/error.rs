use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("routine not found")]
    NotFound,
    #[error("explicit user confirmation required")]
    Confirmation,
    #[error("transit provider unavailable; no live estimate available")]
    Provider,
    #[error("storage unavailable")]
    Storage(#[from] sqlx::Error),
    #[error("stored data invalid")]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
pub const PUBLIC_USER: &str = "__waynudge_public__";
pub fn private_access(user: &str) -> Result<()> {
    if user == PUBLIC_USER {
        Err(Error::Unauthorized)
    } else {
        Ok(())
    }
}
#[derive(Serialize)]
struct ErrorBody {
    error: String,
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Confirmation => StatusCode::CONFLICT,
            Self::Provider => StatusCode::BAD_GATEWAY,
            Self::Storage(_) | Self::Json(_) => {
                tracing::error!("storage operation failed");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (
            status,
            Json(ErrorBody {
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}
pub fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}
pub fn confirmed(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(Error::Confirmation)
    }
}
