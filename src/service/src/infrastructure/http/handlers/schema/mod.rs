use crate::application::AppState;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::schema::dto::{
    DetailedDocumentResponse, DocumentResponse,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use luminair_common::DocumentTypeId;

mod dto;

pub async fn documents_metadata<S: AppState>(
    State(state): State<S>,
) -> Result<ApiSuccess<Vec<DocumentResponse>>, ApiError> {
    let result = state
        .document_types()
        .iterate()
        .map(DocumentResponse::from)
        .collect::<Vec<_>>();

    Ok(ApiSuccess::new(StatusCode::OK, result))
}

pub async fn one_document_metadata<S: AppState>(
    Path(id): Path<String>,
    State(state): State<S>,
) -> Result<ApiSuccess<DetailedDocumentResponse>, ApiError> {
    let document_type_id = DocumentTypeId::try_new(id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let result = state
        .document_types()
        .get(&document_type_id)
        .map(DetailedDocumentResponse::from)
        .ok_or_else(|| {
            ApiError::NotFound(format!("Document type metadata for ID '{}' not found", id))
        })?;

    Ok(ApiSuccess::new(StatusCode::OK, result))
}
