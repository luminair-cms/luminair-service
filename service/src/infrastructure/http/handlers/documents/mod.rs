use crate::domain::AppState;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::documents::dto::{DetailedDocumentResponse, DocumentResponse};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use luminair_common::DocumentTypeId;

mod dto;

pub async fn documents_metadata(
    State(state): State<AppState>,
) -> Result<ApiSuccess<Vec<DocumentResponse>>, ApiError> {
    let result = state
        .schema_registry
        .iterate()
        .map(DocumentResponse::from)
        .collect::<Vec<_>>();

    Ok(ApiSuccess::new(StatusCode::OK, result))
}

pub async fn one_document_metadata(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<ApiSuccess<DetailedDocumentResponse>, ApiError> {
    let document_id =
        DocumentTypeId::try_new(id).map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let result = state
        .schema_registry
        .get(&document_id)
        .map(DetailedDocumentResponse::from);

    if let Some(document) = result {
        Ok(ApiSuccess::new(StatusCode::OK, document))
    } else {
        Err(ApiError::NotFound)
    }
}
