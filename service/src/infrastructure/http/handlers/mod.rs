use axum::extract::State;
use axum::http::StatusCode;
use crate::domain::{AppState, HelloService};
use crate::infrastructure::http::api::{ApiError, ApiSuccess};

pub mod documents;
mod data;

// health check handler
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}

// hello world from db
pub async fn hello_world_handler<S: AppState>(
    State(state): State<S>,
) -> Result<ApiSuccess<String>, ApiError> {
    state
        .hello_service()
        .hello()
        .await
        .map_err(ApiError::from)
        .map(|result| ApiSuccess::new(StatusCode::OK, result))
}
