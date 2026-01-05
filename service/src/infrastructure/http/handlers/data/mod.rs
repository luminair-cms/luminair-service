use crate::domain::{AppState, Persistence, QueryBuilder, ResultSet};
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::dto::{
    DocumentRowResponse, ManyDocumentRowsResponse, MetadataResponse, OneDocumentRowResponse,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use luminair_common::domain::DocumentId;

mod dto;

pub async fn find_document_by_id<S: AppState>(
    Path(document_id): Path<String>,
    Path(id): Path<String>,
    State(state): State<S>,
) -> Result<ApiSuccess<OneDocumentRowResponse>, ApiError> {
    let document_id = DocumentId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let document_metadata = state.documents().get_persisted_document(&document_id);

    if document_metadata.is_none() {
        return Err(ApiError::NotFound);
    }

    // TODO: given document metadata call documents for create high level request

    Err(ApiError::NotFound)
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

    let query = QueryBuilder::select_all(&document_metadata).generate();

    let result_set = state.persistence().select_all(query).await?;

    use itertools::Itertools;
    
    let data: Vec<DocumentRowResponse> = result_set
        .into_rows()
        .into_iter()
        .into_group_map_by(|row|row.document_id)
        .into_iter()
        .map(DocumentRowResponse::from)
        .collect();
    let meta = MetadataResponse { total: data.len() };
    let result = ManyDocumentRowsResponse { data, meta };

    Ok(ApiSuccess::new(StatusCode::OK, result))
}
