use axum::extract::{Path, State};
use crate::domain::AppState;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::dto::OneDocumentRowResponse;

mod dto;

pub async fn find_document_by_id<S: AppState>(
    Path(document_id): Path<String>,
    Path(id): Path<String>,
    State(state): State<S>
) -> Result<ApiSuccess<OneDocumentRowResponse>, ApiError> {
    Err(ApiError::NotFound)
}