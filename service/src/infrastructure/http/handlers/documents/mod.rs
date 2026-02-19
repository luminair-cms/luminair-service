use std::fmt::format;

use crate::domain::AppState;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::documents::dto::{DetailedDocumentResponse, DocumentResponse};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use luminair_common::DocumentTypeId;

mod dto;

pub async fn documents_metadata<S: AppState>(
    State(state): State<S>,
) -> Result<ApiSuccess<Vec<DocumentResponse>>, ApiError> {
    let result = state
        .document_types_registry()
        .iterate()
        .map(DocumentResponse::from)
        .collect::<Vec<_>>();

    Ok(ApiSuccess::new(StatusCode::OK, result))
}

pub async fn one_document_metadata<S: AppState>(
    Path(id): Path<String>,
    State(state): State<S>,
) -> Result<ApiSuccess<DetailedDocumentResponse>, ApiError> {
    let document_type_id =
        DocumentTypeId::try_new(id).map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let result = state
        .document_types_registry()
        .get(&document_type_id)
        .map(DetailedDocumentResponse::from)
        .ok_or(ApiError::NotFound)?;

    Ok(ApiSuccess::new(StatusCode::OK, result))
}
