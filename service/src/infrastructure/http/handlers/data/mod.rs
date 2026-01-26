use std::collections::{HashMap, HashSet};

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
    pub populate: Option<HashSet<String>>,
}

pub async fn find_document_by_id<S: AppState>(
    Path((document_id, id)): Path<(String, i32)>,
    QueryString(params): QueryString<QueryParams>,
    State(state): State<S>,
) -> Result<ApiSuccess<OneDocumentRowResponse>, ApiError> {
    let document_id = DocumentId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let documents = state.documents();
    let persistence = state.persistence();

    let document_metadata = documents
        .get_document(&document_id)
        .ok_or(ApiError::NotFound)?;

    let query = QueryBuilder::from(document_metadata).find_by_document_id();
    let result_set = persistence.select_by_id(query, id).await?;

    let mut data = result_set_into_document_response(result_set);

    if let Some(relations_to_populate) = params.populate {
        // map from attribute_id to vector of related documents
        let mut populated_relations = HashMap::new();

        for relation_to_populate in relations_to_populate.iter() {
            let attribute_id = AttributeId::try_new(relation_to_populate)
                .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

            let relation = document_metadata.relations.get(&attribute_id).ok_or(
                ApiError::UnprocessableEntity(format!(
                    "Attribute {} to populate doesn't exist",
                    relation_to_populate
                )),
            )?;
            let related_document_metadata = documents.get_document(&relation.target).unwrap();

            let query =
                QueryBuilder::from_relation(document_metadata, relation, related_document_metadata)
                    .with_owning_id_condition(&document_metadata.persistence.relation_column_name)
                    .into();
            let related_result_set = persistence.select_by_id(query, id).await?;
            let related_data = result_set_into_document_response(related_result_set);
            populated_relations.insert(attribute_id.to_string(), related_data);
        }

        data = join_relations(data, populated_relations);
    }

    OneDocumentRowResponse::try_from(data)
        .map(|result| ApiSuccess::new(StatusCode::OK, result))
        .map_err(|_| ApiError::NotFound)
}

pub async fn find_all_documents<S: AppState>(
    Path(document_id): Path<String>,
    QueryString(params): QueryString<QueryParams>,
    State(state): State<S>,
) -> Result<ApiSuccess<ManyDocumentRowsResponse>, ApiError> {
    let document_id = DocumentId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let documents = state.documents();
    let persistence = state.persistence();

    let document_metadata = documents
        .get_document(&document_id)
        .ok_or(ApiError::NotFound)?;

    let query: Query = QueryBuilder::from(document_metadata).into();

    let result_set = persistence.select_all(query).await?;

    let mut data = result_set_into_document_response(result_set);

    if let Some(relations_to_populate) = params.populate {
        let mut populated_relations = HashMap::new();

        for relation_to_populate in relations_to_populate.iter() {
            let attribute_id = AttributeId::try_new(relation_to_populate)
                .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

            let relation = document_metadata.relations.get(&attribute_id).ok_or(
                ApiError::UnprocessableEntity(format!(
                    "Attribute {} to populate doesn't exist",
                    relation_to_populate
                )),
            )?;
            let related_document_metadata = documents.get_document(&relation.target).unwrap();

            let query =
                QueryBuilder::from_relation(document_metadata, relation, related_document_metadata)
                    .into();
            let related_result_set = persistence.select_all(query).await?;
            let related_data = result_set_into_document_response(related_result_set);
            populated_relations.insert(attribute_id.to_string(), related_data);
        }

        data = join_relations(data, populated_relations);
    }

    Ok(ApiSuccess::new(
        StatusCode::OK,
        ManyDocumentRowsResponse::from(data),
    ))
}

fn result_set_into_document_response(result_set: impl ResultSet) -> Vec<DocumentRowResponse> {
    result_set
        .into_rows()
        .into_iter()
        .map(DocumentRowResponse::from)
        .collect()
}

// join relations to the main document response
// relations: map from attribute_id to vector of related documents;
// each of the related documents will be added to the main document response by its owning_id
fn join_relations(
    main: Vec<DocumentRowResponse>,
    relations: HashMap<String, Vec<DocumentRowResponse>>,
) -> Vec<DocumentRowResponse> {
    // map from: attribute_id to: map from document_id to vector of related documents
    let mut transposed: HashMap<String, HashMap<i32, Vec<DocumentRowResponse>>> = HashMap::new();

    for (attribute_id, related_docs) in relations {
        let mut grouped: HashMap<i32, Vec<DocumentRowResponse>> = HashMap::new();
        for doc in related_docs {
            if let Some(owning_id) = doc.owning_id {
                grouped.entry(owning_id).or_default().push(doc);
            }
        }
        transposed.insert(attribute_id, grouped);
    }

    main.into_iter()
        .map(|mut document_response| {
            for (attribute_id, grouped) in &transposed {
                if let Some(related_docs) = grouped.get(&document_response.document_id) {
                    document_response.add_relation(attribute_id.clone(), related_docs.clone());
                } else {
                    document_response.add_relation(attribute_id.clone(), Vec::new());
                }
            }
            document_response
        })
        .collect()
}
