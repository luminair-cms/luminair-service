use std::collections::HashSet;

use crate::domain::query::{Query, QueryBuilder};
use crate::domain::{AppState, Persistence, ResultSet};
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::dto::{
    DocumentRowResponse, ManyDocumentRowsResponse, OneDocumentRowResponse,
};
use crate::infrastructure::http::querystring::QueryString;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use luminair_common::domain::{AttributeId, DocumentId};
use serde::Deserialize;

mod dto;

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    pub populate: Option<HashSet<String>>
}

pub async fn find_document_by_id<S: AppState>(
    Path((document_id,id)): Path<(String,i32)>,
    QueryString(params): QueryString<QueryParams>,
    State(state): State<S>,
) -> Result<ApiSuccess<OneDocumentRowResponse>, ApiError> {
    let document_id = DocumentId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let documents = state.documents();
    let persistence = state.persistence();
    
    let document_metadata = documents
        .get_persisted_document(&document_id)
        .ok_or(ApiError::NotFound)?;
    
    let query = QueryBuilder::from(document_metadata).find_by_document_id();
    let result_set = persistence.select_by_id(query, id).await?;
    
    if let Some(relations_to_populate) = params.populate {
        for relation_to_populate in relations_to_populate.iter() {
            let attribute_id = AttributeId::try_new(relation_to_populate)
                .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;
            
            let relation = document_metadata.relations.get(&attribute_id)
                .ok_or(ApiError::UnprocessableEntity(format!("Attribute {} to populate doesn't exist", relation_to_populate)))?;
            let related_document_metadata = documents.get_persisted_document_by_ref(relation.target).unwrap();
            
            let query = QueryBuilder::from_relation(document_metadata, relation, related_document_metadata);
            let related_result_set = persistence.select_by_id(query, id).await?;
            let related_data = result_set_into_document_response(related_result_set);
        }
    }
    
    let data = result_set_into_document_response(result_set);
    
    OneDocumentRowResponse::try_from(data)
        .map(|result|ApiSuccess::new(StatusCode::OK, result))
        .map_err(|_|ApiError::NotFound)
}

pub async fn find_all_documents<S: AppState>(
    Path(document_id): Path<String>,
    QueryString(params): QueryString<QueryParams>,
    State(state): State<S>,
) -> Result<ApiSuccess<ManyDocumentRowsResponse>, ApiError> {
    let document_id = DocumentId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let document_metadata = state
        .documents()
        .get_persisted_document(&document_id)
        .ok_or(ApiError::NotFound)?;

    let query: Query = QueryBuilder::from(document_metadata).into();

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