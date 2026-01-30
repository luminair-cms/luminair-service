use std::collections::{HashMap, HashSet};

use crate::domain::query::{Query, QueryBuilder, QueryPagination};
use crate::domain::{AppState, DocumentRowId, Persistence, ResultSet};
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::dto::{DocumentRowResponse, GroupedDocumentRowResponse, ManyDocumentRowsResponse, OneDocumentRowResponse};
use crate::infrastructure::http::querystring::QueryString;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use itertools::{EitherOrBoth, Itertools};
use luminair_common::domain::{AttributeId, DocumentId};
use serde::Deserialize;

mod dto;

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    pub populate: Option<HashSet<String>>,
    pub pagination: Option<PaginationParams>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PaginationParams {
    #[serde(default)]
    pub page: u16,
    #[serde(default)]
    pub page_ize: u16
}

pub async fn find_document_by_id<S: AppState>(
    Path((document_id, id)): Path<(String, i32)>,
    QueryString(params): QueryString<QueryParams>,
    State(state): State<S>,
) -> Result<ApiSuccess<OneDocumentRowResponse>, ApiError> {
    if params.pagination.is_some() {
        return Err(ApiError::UnprocessableEntity("Pagination param isn't eligible for find_by_id query".to_string()));
    }
    
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
                    .with_owning_id(&document_metadata.persistence.relation_column_name)
                    .into();
            let related_result_set = persistence.select_by_id(query, id).await?;
            let related_data = result_set_into_groped_document_response(related_result_set);
            populated_relations.insert(attribute_id, related_data);
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
    
    let pagination = params.pagination
        .map_or_else(
            || QueryPagination::default(), 
            |it| QueryPagination::new(it.page, it.page_ize));

    let query: Query = QueryBuilder::from(document_metadata)
        .with_pagination(pagination).into();

    let result_set = persistence.select_all(query).await?;

    let mut data = result_set_into_document_response(result_set);

    if let Some(relations_to_populate) = params.populate {
        let ids: Vec<i32> = data.iter().map(|row| row.document_id.into()).collect();
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
                    .with_owning_id_list(&document_metadata.persistence.relation_column_name)
                    .into();
            let related_result_set = persistence.select_by_id_list(query, &ids).await?;
            let related_data = result_set_into_groped_document_response(related_result_set);
            populated_relations.insert(attribute_id, related_data);
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

fn result_set_into_groped_document_response(result_set: impl ResultSet) -> Vec<GroupedDocumentRowResponse> {
    result_set
        .into_rows()
        .into_iter()
        .filter_map(|row| {
            let owning_id: DocumentRowId = row.owning_id.map(|it| it.into())?;
            Some((owning_id, DocumentRowResponse::from(row)))
        })
        .chunk_by(|(owning_id, _)| owning_id.clone())
        .into_iter()
        .map(|(owning_id, group)| GroupedDocumentRowResponse {
            owning_id,
            rows: group.map(|(_, row)| row).collect(),
        })
        .collect()
}

// join relations to the main document response
// relations: map from attribute_id to vector of related documents;
// each of the related documents will be added to the main document response by its owning_id
// both main and relations are sorted by owning DocumentRowId - document_id of main rows
fn join_relations(
    main: Vec<DocumentRowResponse>,
    relations: HashMap<AttributeId, Vec<GroupedDocumentRowResponse>>,
) -> Vec<DocumentRowResponse> {
    let mut transposed_map: HashMap<DocumentRowId, HashMap<AttributeId, Vec<DocumentRowResponse>>> =
        HashMap::new();

    for (attribute_id, grouped_rows) in relations {
        for grouped_row in grouped_rows {
            transposed_map
                .entry(grouped_row.owning_id)
                .or_default()
                .insert(attribute_id.clone(), grouped_row.rows);
        }
    }

    let mut transposed: Vec<(
        DocumentRowId,
        HashMap<AttributeId, Vec<DocumentRowResponse>>,
    )> = transposed_map.into_iter().collect();
    transposed.sort_by(|(a, _), (b, _)| a.cmp(b));

    let transposed_iter = transposed.into_iter();

    let joined = main.into_iter()
        .merge_join_by(transposed_iter, |a,(b,_)| a.document_id.cmp(b));

    let mut enriched_documents = Vec::new();

    for item in joined {
        match item {
            EitherOrBoth::Both(entity, (_id, populated)) => {
                enriched_documents.push(entity.with_relations(populated));
            }
            EitherOrBoth::Left(entity) => {
                enriched_documents.push(entity);
            }
            EitherOrBoth::Right((_id, populated)) => {
                println!("Doc without entity: {:?}", populated);
            }
        }
    }

    enriched_documents
}
