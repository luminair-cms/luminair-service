use crate::domain::query::QueryBuilder;
use crate::domain::{AppState, Persistence, ResultSet};
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::dto::{
    DocumentRowResponse, ManyDocumentRowsResponse, OneDocumentRowResponse,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use luminair_common::domain::DocumentId;

mod dto;

pub async fn find_document_by_id<S: AppState>(
    Path((document_id,id)): Path<(String,i32)>,
    State(state): State<S>,
) -> Result<ApiSuccess<OneDocumentRowResponse>, ApiError> {
    let document_id = DocumentId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let document_metadata = state
        .documents()
        .get_persisted_document(&document_id)
        .ok_or(ApiError::NotFound)?;
    
    let query = QueryBuilder::new(document_metadata).find_by_id().build();
    let result_set = state.persistence().select_by_id(query, id).await?;
    
    let data = result_set_into_document_response(result_set);
    
    OneDocumentRowResponse::try_from(data)
        .map(|result|ApiSuccess::new(StatusCode::OK, result))
        .map_err(|_|ApiError::NotFound)
}

pub async fn find_all_documents<S: AppState>(
    Path(document_id): Path<String>,
    State(state): State<S>,
) -> Result<ApiSuccess<ManyDocumentRowsResponse>, ApiError> {
    let document_id = DocumentId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let document_metadata = state
        .documents()
        .get_persisted_document(&document_id)
        .ok_or(ApiError::NotFound)?;

    let query = QueryBuilder::new(&document_metadata).build();
    let result_set = state.persistence().select_all(query).await?;
    let data = result_set_into_document_response(result_set);

    Ok(ApiSuccess::new(StatusCode::OK, ManyDocumentRowsResponse::from(data)))
}

fn result_set_into_document_response(result_set: impl ResultSet) -> Vec<DocumentRowResponse> {
    use itertools::Itertools;
    
    result_set
        .into_rows()
        .into_iter()
        .into_group_map_by(|row|row.document_id)
        .into_iter()
        .map(DocumentRowResponse::from)
        .collect()
}