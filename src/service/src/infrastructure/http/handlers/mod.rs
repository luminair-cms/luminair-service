use axum::http::StatusCode;

pub mod content;
pub mod schema;

// health check handler
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
