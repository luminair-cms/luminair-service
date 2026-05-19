use crate::domain::AppState;
use crate::domain::application::{DocumentsService};
use crate::domain::document::{DocumentInstanceId};
use crate::domain::repository::query::{DocumentInstanceQuery, DocumentStatus};
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::response::{ManyDocumentsResponse, OneDocumentResponse};

use crate::infrastructure::http::querystring::QueryString;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::collections::HashSet;
use std::str::FromStr;
use luminair_common::DocumentTypeApiId;

mod request;
mod response;

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    /// A set of attribute IDs to populate in the response. If not provided, no relations will be populated.
    pub populate: Option<HashSet<String>>,
    /// Pagination parameters. Only eligible for find_all_documents query, not for find_by_id query.
    /// If not provided, defaults to page=1 and page_size=25.
    pub pagination: Option<PaginationParams>,
    /// Document publication status: "published" (default) or "draft"
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "published".to_string()
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

    let api_id = DocumentTypeApiId::from_str(&api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    let document_type = state.document_types().lookup(&api_id)
        .ok_or(ApiError::NotFound)?;

    let document_instance_id = DocumentInstanceId::try_from(&id)?;

    let populate_attributes = if let Some(populate_fields) = params.populate {
        let mut populate_attributes = Vec::with_capacity(populate_fields.len());
        for name in populate_fields {
            let attr = luminair_common::AttributeId::try_new(&name).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid populate field: {}", name))
            })?;
            populate_attributes.push(attr);
        }
        Some(populate_attributes)
    } else {
        None
    };

    let status = match params.status.as_str() {
        "draft" => DocumentStatus::Draft,
        "published" => DocumentStatus::Published,
        _ => return Err(ApiError::UnprocessableEntity(
            "status must be 'published' (default) or 'draft'".to_string(),
        )),
    };

    let query = DocumentInstanceQuery::new().with_status(status);

    let document_instance = state
        .documents_service()
        .find_by_id(document_type, populate_attributes, query, document_instance_id)
        .await
        .map_err(|err| ApiError::from(err))?;

    OneDocumentResponse::try_from(document_instance)
        .map(|response|
            ApiSuccess::new(StatusCode::OK, response)
        ).map_err(|_| ApiError::NotFound)
}

pub async fn find_all_documents<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<ManyDocumentsResponse>, ApiError> {
    let api_id = DocumentTypeApiId::from_str(&api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    let document_type = state.document_types().lookup(&api_id)
        .ok_or(ApiError::NotFound)?;

    // Extract pagination params with defaults
    let (page, page_size) = params
        .pagination
        .map(|p| (p.page, p.page_size))
        .unwrap_or((1, 25));

    let populate_attributes = if let Some(populate_fields) = params.populate {
        let mut populate_attributes = Vec::with_capacity(populate_fields.len());
        for name in populate_fields {
            let attr = luminair_common::AttributeId::try_new(&name).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid populate field: {}", name))
            })?;
            populate_attributes.push(attr);
        }
        Some(populate_attributes)
    } else {
        None
    };

    // Parse status parameter and convert to include_drafts
    let status = match params.status.as_str() {
        "draft" => DocumentStatus::Draft,
        "published" => DocumentStatus::Published,
        _ => return Err(ApiError::UnprocessableEntity(
            "status must be 'published' (default) or 'draft'".to_string(),
        )),
    };

    // Build query using builder pattern - pagination guards are enforced by the query
    let query = DocumentInstanceQuery::new()
        .paginate(page, page_size)
        .with_status(status);

    let documents: Vec<response::DocumentInstanceResponse> = state
        .documents_service()
        .find(document_type, populate_attributes, query)
        .await
        .map_err(|err| ApiError::from(err))?
        .into_iter()
        .map(Into::into)
        .collect();

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
    let api_id = DocumentTypeApiId::from_str(&api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    let document_type = state.document_types().lookup(&api_id)
        .ok_or(ApiError::NotFound)?;

    // Validate and build fields from the payload using document type metadata
    let fields = request::build_fields_from_payload(document_type, &payload)
        .map_err(|err| ApiError::UnprocessableEntity(err.to_string()))?;

    let created_document_id = state
        .documents_service()
        .create(document_type, fields)
        .await
        .map_err(|err| ApiError::from(err))?;

    let created_id: String = created_document_id.into();

    let location = format!("/api/documents/{}/{}", api_type, created_id);
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::LOCATION,
        axum::http::HeaderValue::from_str(&location)
            .map_err(|_| ApiError::InternalServerError("Invalid location header".to_string()))?,
    );

    Ok((StatusCode::CREATED, headers))
}

pub async fn delete_existing_document<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let api_id = DocumentTypeApiId::from_str(&api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    let document_type = state.document_types().lookup(&api_id)
        .ok_or(ApiError::NotFound)?;
    
    let instance_id = DocumentInstanceId::try_from(&id)?;

    state
        .documents_service()
        .delete(document_type, instance_id)
        .await
        .map_err(|err| ApiError::from(err))?;

    Ok((StatusCode::NO_CONTENT, ()))
}

/// Request body for connect/disconnect operations
#[derive(Deserialize, Debug)]
pub struct RelationModifyRequest {
    pub data: serde_json::Value,
}

/// Handle connect/disconnect relation operations
pub async fn modify_relations<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
    Json(payload): Json<RelationModifyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let api_id = DocumentTypeApiId::from_str(&api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    let document_type = state.document_types().lookup(&api_id)
        .ok_or(ApiError::NotFound)?;

    // Parse main document ID
    let main_instance_id = DocumentInstanceId::try_from(&id)?;
    
    // Fetch main document to get its DatabaseRowId
    let main_docs = state
        .documents_service()
        .find_by_id(
            document_type,
            None,
            DocumentInstanceQuery::new(),
            main_instance_id,
        )
        .await
        .map_err(|err| ApiError::from(err))?;

    let main_doc = main_docs.first().ok_or(ApiError::NotFound)?;
    let owning_row_id = main_doc.id;

    // Extract relation field operations from data object
    let data_obj = payload
        .data
        .as_object()
        .ok_or(ApiError::UnprocessableEntity("data must be an object".to_string()))?;

    // Process each relation field
    for (field_name, field_value) in data_obj {
        let attr_id = luminair_common::AttributeId::try_new(field_name)
            .map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid relation field: {}", field_name))
            })?;

        let relation_metadata = document_type.relations.get(&attr_id).ok_or_else(|| {
            ApiError::UnprocessableEntity(format!("Relation not found: {}", field_name))
        })?;

        if !relation_metadata.relation_type.is_owning() {
            return Err(ApiError::UnprocessableEntity(format!(
                "Relation {} is not owning",
                field_name
            )));
        }

        let related_document_type = state
            .document_types()
            .get(&relation_metadata.target)
            .ok_or(ApiError::NotFound)?;

        let field_obj = field_value
            .as_object()
            .ok_or(ApiError::UnprocessableEntity(
                format!("Field {} must be an object", field_name),
            ))?;

        // Process connect operations
        if let Some(connect_list) = field_obj.get("connect") {
            let connect_ids = parse_document_id_list(connect_list)?;
            for doc_id_str in connect_ids {
                let related_instance_id = DocumentInstanceId::try_from(&doc_id_str)?;
                let related_docs = state
                    .documents_service()
                    .find_by_id(
                        related_document_type,
                        None,
                        DocumentInstanceQuery::new(),
                        related_instance_id,
                    )
                    .await
                    .map_err(|err| ApiError::from(err))?;

                let related_doc = related_docs.first().ok_or(ApiError::NotFound)?;
                let inverse_row_id = related_doc.id;

                state
                    .documents_service()
                    .connect(document_type, &attr_id, owning_row_id, inverse_row_id)
                    .await
                    .map_err(|err| ApiError::from(err))?;
            }
        }

        // Process disconnect operations
        if let Some(disconnect_list) = field_obj.get("disconnect") {
            let disconnect_ids = parse_document_id_list(disconnect_list)?;
            for doc_id_str in disconnect_ids {
                let related_instance_id = DocumentInstanceId::try_from(&doc_id_str)?;
                let related_docs = state
                    .documents_service()
                    .find_by_id(
                        related_document_type,
                        None,
                        DocumentInstanceQuery::new(),
                        related_instance_id,
                    )
                    .await
                    .map_err(|err| ApiError::from(err))?;

                let related_doc = related_docs.first().ok_or(ApiError::NotFound)?;
                let inverse_row_id = related_doc.id;

                state
                    .documents_service()
                    .disconnect(document_type, &attr_id, owning_row_id, inverse_row_id)
                    .await
                    .map_err(|err| ApiError::from(err))?;
            }
        }
    }

    Ok((StatusCode::NO_CONTENT, ()))
}

/// Parse document IDs from either shorthand or longhand format
fn parse_document_id_list(value: &serde_json::Value) -> Result<Vec<String>, ApiError> {
    match value {
        serde_json::Value::Array(arr) => {
            let mut ids = Vec::new();
            for item in arr {
                match item {
                    // Shorthand: direct string ID
                    serde_json::Value::String(id) => ids.push(id.clone()),
                    // Longhand: object with documentId field
                    serde_json::Value::Object(obj) => {
                        let id = obj
                            .get("documentId")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ApiError::UnprocessableEntity(
                                    "documentId must be a string".to_string(),
                                )
                            })?;
                        ids.push(id.to_string());
                    }
                    _ => {
                        return Err(ApiError::UnprocessableEntity(
                            "Each item must be a string or object with documentId".to_string(),
                        ))
                    }
                }
            }
            Ok(ids)
        }
        _ => Err(ApiError::UnprocessableEntity(
            "connect/disconnect must be an array".to_string(),
        )),
    }
}
