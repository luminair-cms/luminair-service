use std::collections::HashSet;

use crate::domain::AppState;
use crate::domain::document::DocumentInstanceId;
use crate::domain::repository::DocumentInstanceRepository;
use crate::domain::repository::query::DocumentInstanceQuery;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::dto::{ManyDocumentsResponse, OneDocumentResponse};
use crate::infrastructure::http::querystring::QueryString;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use luminair_common::DocumentTypeId;
use serde::Deserialize;

mod dto;

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    pub populate: Option<HashSet<String>>,
    pub pagination: Option<PaginationParams>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PaginationParams {
    #[serde(default)]
    pub page: u16,
    #[serde(default)]
    pub page_size: u16,
}

pub async fn find_document_by_id<S: AppState>(
    State(state): State<S>,
    Path((document_id, id)): Path<(String, i64)>,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<OneDocumentResponse>, ApiError> {
    if params.pagination.is_some() {
        return Err(ApiError::UnprocessableEntity(
            "Pagination param isn't eligible for find_by_id query".to_string(),
        ));
    }
    let document_type_id = DocumentTypeId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let repository = state.documents_instance_repository();

    let result = repository
        .find_by_id(document_type_id, DocumentInstanceId::from(id))
        .await
        .map_err(|err| ApiError::from(err))?;

    OneDocumentResponse::try_from(result)
        .map(|result| ApiSuccess::new(StatusCode::OK, result))
        .map_err(|_| ApiError::NotFound)
}

pub async fn find_all_documents<S: AppState>(
    State(state): State<S>,
    Path(document_id): Path<String>,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<ManyDocumentsResponse>, ApiError> {
    let document_type_id = DocumentTypeId::try_new(document_id)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let repository = state.documents_instance_repository();

    // Extract pagination params with defaults
    let (page, page_size) = params
        .pagination
        .map(|p| (p.page, p.page_size))
        .unwrap_or((1, 25));

    // Build query using builder pattern - pagination guards are enforced by the query
    let query = DocumentInstanceQuery::new(document_type_id).paginate(page, page_size);

    let result = crate::domain::repository::DocumentInstanceRepository::find(repository, query)
        .await
        .map_err(|err| ApiError::from(err))?;

    Ok(ApiSuccess::new(
        StatusCode::OK,
        ManyDocumentsResponse::from(result),
    ))
}

/*
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
 */
