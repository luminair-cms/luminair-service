use axum::http::StatusCode;

pub mod documents;
pub mod data;

// health check handler
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
