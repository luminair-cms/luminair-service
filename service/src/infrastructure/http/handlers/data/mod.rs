use std::collections::HashSet;

use crate::domain::AppState;
use crate::domain::document::lifecycle::PublicationState;
use crate::domain::document::{DatabaseRowId, DocumentContent, DocumentInstanceId};
use crate::domain::repository::DocumentInstanceRepository;
use crate::domain::repository::query::DocumentInstanceQuery;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::response::{
    ManyDocumentsResponse, OneDocumentResponse,
};
use crate::infrastructure::http::querystring::QueryString;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

mod request;
mod response;

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    /// A set of attribute IDs to populate in the response. If not provided, no relations will be populated.
    pub populate: Option<HashSet<String>>,
    /// Pagination parameters. Only eligible for find_all_documents query, not for find_by_id query.
    /// If not provided, defaults to page=1 and page_size=25.
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
    Path((api_type, id)): Path<(String, String)>,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<OneDocumentResponse>, ApiError> {
    if params.pagination.is_some() {
        return Err(ApiError::UnprocessableEntity(
            "Pagination param isn't eligible for find_by_id query".to_string(),
        ));
    }

    let document_type = state
        .document_type_index()
        .lookup(&api_type)
        .ok_or(ApiError::NotFound)?;

    let document_instance_id = DocumentInstanceId::try_from(&id)?;
    let repository = state.documents_instance_repository();

    let mut document_response: response::DocumentInstanceResponse = repository
        .find_by_id(document_type, document_instance_id)
        .await
        .map_err(|err| ApiError::from(err))?
        .ok_or(ApiError::NotFound)?
        .into();

    // Apply populate if requested
    if let Some(populate_fields) = params.populate {
        // validate each attribute id
        let mut attr_ids = Vec::with_capacity(populate_fields.len());
        for name in populate_fields {
            let attr = luminair_common::AttributeId::try_new(&name).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid populate field: {}", name))
            })?;
            attr_ids.push(attr);
        }
        let main_table_id = DatabaseRowId::from(document_response.id);
        let relations = repository
            .fetch_relations_for_one(&document_type, main_table_id, &attr_ids)
            .await
            .map_err(|err| ApiError::from(err))?;

        // Convert relation instances to responses
        let relations_mapped: std::collections::HashMap<
            luminair_common::AttributeId,
            Vec<response::DocumentInstanceResponse>,
        > = relations
            .into_iter()
            .map(|(attr_id, instances)| {
                let responses: Vec<response::DocumentInstanceResponse> =
                    instances.into_iter().map(Into::into).collect();
                (attr_id, responses)
            })
            .collect();

        document_response = document_response.with_relations(relations_mapped);
    }

    Ok(ApiSuccess::new(
        StatusCode::OK,
        OneDocumentResponse {
            data: document_response,
        },
    ))
}

pub async fn find_all_documents<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<ManyDocumentsResponse>, ApiError> {
    let document_type = state
        .document_type_index()
        .lookup(&api_type)
        .ok_or(ApiError::NotFound)?;

    let repository = state.documents_instance_repository();

    // Extract pagination params with defaults
    let (page, page_size) = params
        .pagination
        .map(|p| (p.page, p.page_size))
        .unwrap_or((1, 25));

    // Build query using builder pattern - pagination guards are enforced by the query
    let query = DocumentInstanceQuery::new().paginate(page, page_size);

    let mut documents: Vec<response::DocumentInstanceResponse> = repository
        .find(document_type, query)
        .await
        .map_err(|err| ApiError::from(err))?
        .into_iter()
        .map(Into::into)
        .collect();

    // Apply populate if requested
    if !documents.is_empty() {
        if let Some(populate_fields) = params.populate {
            // convert and validate attribute IDs
            let mut attr_ids = Vec::with_capacity(populate_fields.len());
            for name in populate_fields {
                let attr = luminair_common::AttributeId::try_new(&name).map_err(|_| {
                    ApiError::UnprocessableEntity(format!("Invalid populate field: {}", name))
                })?;
                attr_ids.push(attr);
            }

            // Collect all instance IDs for batch fetching
            let ids: Vec<DatabaseRowId> = documents
                .iter()
                .map(|doc| DatabaseRowId::try_from(doc.id).unwrap())
                .collect();

            // Fetch all relations for this batch of documents
            let all_relations = repository
                .fetch_relations_for_many(document_type, &ids, &attr_ids)
                .await
                .map_err(|err| ApiError::from(err))?;

            // Apply relations to each document response
            for doc_response in &mut documents {
                let id = DatabaseRowId::from(doc_response.id);

                let doc_relations: std::collections::HashMap<
                    luminair_common::AttributeId,
                    Vec<response::DocumentInstanceResponse>,
                > = all_relations
                    .iter()
                    .filter_map(|(attr_id, related_docs_by_id)| {
                        let related_responses: Vec<response::DocumentInstanceResponse> =
                            related_docs_by_id
                                .get(&id)
                                .map(|instances| {
                                    instances.iter().cloned().map(Into::into).collect()
                                })
                                .unwrap_or_default();
                        Some((attr_id.clone(), related_responses))
                    })
                    .collect();

                *doc_response = doc_response.clone().with_relations(doc_relations);
            }
        }
    }

    let total = documents.len();
    Ok(ApiSuccess::new(
        StatusCode::OK,
        ManyDocumentsResponse {
            data: documents,
            meta: response::MetadataResponse { total },
        },
    ))
}

pub async fn create_new_document<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
   Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
       let document_type = state
           .document_type_index()
           .lookup(&api_type)
           .ok_or(ApiError::NotFound)?;
   
       let repository = state.documents_instance_repository();
       
       // Validate and build fields from payload using document type metadata
        let fields = request::build_fields_from_payload(document_type, &payload)
            .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;
           
       let content = DocumentContent {
               fields,
               publication_state: PublicationState::Draft { 
                   revision: 0 
               },
           };
       
       let created_document_id = repository
           .create(document_type, content, None)
           .await
           .map_err(|err| ApiError::from(err))?;
       
       let created_id: String = created_document_id.into();
       
        let location = format!("/documents/{}/{}", api_type, created_id);
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::LOCATION,
            axum::http::HeaderValue::from_str(&location)
                .map_err(|_| ApiError::InternalServerError("Invalid location header".to_string()))?
        );
    
        Ok((
            StatusCode::CREATED,
            headers,
        ))
}
