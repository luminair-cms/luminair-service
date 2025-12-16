use axum::extract::{Path, State};
use luminair_common::domain::DocumentId;
use crate::domain::AppState;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::dto::{ManyDocumentRowsResponse, OneDocumentRowResponse};

mod dto;

pub async fn find_document_by_id<S: AppState>(
    Path(document_id): Path<String>,
    Path(id): Path<String>,
    State(state): State<S>
) -> Result<ApiSuccess<OneDocumentRowResponse>, ApiError> {
    let document_id =
        DocumentId::try_new(document_id).map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;
    
    let document_metadata = state
        .documents()
        .get_document(&document_id);
    
    if document_metadata.is_none() {
        return Err(ApiError::NotFound);
    }
    
    // TODO: goven document metadata call documents for create high level request

    Err(ApiError::NotFound)
}

pub async fn find_all_documents<S: AppState>(
    Path(document_id): Path<String>,
    State(state): State<S>
) -> Result<ApiSuccess<ManyDocumentRowsResponse>, ApiError> {
    let document_id =
        DocumentId::try_new(document_id).map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;
    
    let document_metadata = state
        .documents()
        .get_document(&document_id);
    
    if document_metadata.is_none() {
        return Err(ApiError::NotFound);
    }
    
    // TODO: goven document metadata call documents for create high level request
    
    Err(ApiError::NotFound)
}