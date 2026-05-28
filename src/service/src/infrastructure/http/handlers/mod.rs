use axum::http::StatusCode;

pub mod schema;
pub mod content;

// health check handler
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
