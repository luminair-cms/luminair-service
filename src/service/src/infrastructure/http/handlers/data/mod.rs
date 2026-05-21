use crate::domain::application::AppState;
use crate::domain::application::DocumentsService;
use crate::domain::document::DocumentInstanceId;
use crate::domain::query::{DocumentInstanceQuery, DocumentStatus};
use crate::domain::repository::RelationOps;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::data::response::{
    ManyDocumentsResponse, OneDocumentResponse,
};
use crate::infrastructure::http::querystring::QueryString;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use luminair_common::DocumentTypeApiId;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;

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
    let document_type = state
        .document_types()
        .lookup(&api_id)
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
        _ => {
            return Err(ApiError::UnprocessableEntity(
                "status must be 'published' (default) or 'draft'".to_string(),
            ));
        }
    };

    let query = DocumentInstanceQuery::new().with_status(status);

    let document_instance = state
        .documents_service()
        .find_by_id(
            document_type,
            populate_attributes,
            query,
            document_instance_id,
        )
        .await
        .map_err(|err| ApiError::from(err))?;

    OneDocumentResponse::try_from(document_instance)
        .map(|response| ApiSuccess::new(StatusCode::OK, response))
        .map_err(|_| ApiError::NotFound)
}

pub async fn find_all_documents<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<ManyDocumentsResponse>, ApiError> {
    let api_id = DocumentTypeApiId::from_str(&api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    let document_type = state
        .document_types()
        .lookup(&api_id)
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
        _ => {
            return Err(ApiError::UnprocessableEntity(
                "status must be 'published' (default) or 'draft'".to_string(),
            ));
        }
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
    let document_type = state
        .document_types()
        .lookup(&api_id)
        .ok_or(ApiError::NotFound)?;

    // Validate and build fields from the payload using document type metadata
    let fields = request::build_fields_from_payload(document_type, &payload).map_err(
        |err: crate::domain::document::error::DocumentError| {
            ApiError::UnprocessableEntity(err.to_string())
        },
    )?;

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
    let document_type = state
        .document_types()
        .lookup(&api_id)
        .ok_or(ApiError::NotFound)?;

    let instance_id = DocumentInstanceId::try_from(&id)?;

    state
        .documents_service()
        .delete(document_type, instance_id)
        .await
        .map_err(|err| ApiError::from(err))?;

    Ok((StatusCode::NO_CONTENT, ()))
}

/// Handle connect/disconnect relation operations.
///
/// Accepts the same `{ "fieldName": { "connect": [...], "disconnect": [...] } }` payload
/// format described in the API docs. Both shorthand (UUID string) and longhand
/// (`{ "documentId": "…" }`) formats are supported for each entry.
pub async fn modify_relations<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
    let api_id = DocumentTypeApiId::from_str(&api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    let document_type = state
        .document_types()
        .lookup(&api_id)
        .ok_or(ApiError::NotFound)?;

    let document_id = DocumentInstanceId::try_from(&id)?;

    let data_obj = payload.as_object().ok_or(ApiError::UnprocessableEntity(
        "body must be a JSON object".into(),
    ))?;

    let mut ops: HashMap<luminair_common::AttributeId, RelationOps> = HashMap::new();

    for (field_name, field_value) in data_obj {
        let attr_id = luminair_common::AttributeId::try_new(field_name).map_err(|_| {
            ApiError::UnprocessableEntity(format!("Invalid relation field: {}", field_name))
        })?;

        let field_obj = field_value.as_object().ok_or_else(|| {
            ApiError::UnprocessableEntity(format!("Field '{}' must be an object", field_name))
        })?;

        let connect = parse_ids_from_list(
            field_obj
                .get("connect")
                .unwrap_or(&serde_json::Value::Array(vec![])),
        )?;
        let disconnect = parse_ids_from_list(
            field_obj
                .get("disconnect")
                .unwrap_or(&serde_json::Value::Array(vec![])),
        )?;

        ops.insert(
            attr_id,
            RelationOps {
                connect,
                disconnect,
            },
        );
    }

    state
        .documents_service()
        .modify_relations(document_type, document_id, ops)
        .await
        .map_err(ApiError::from)?;

    Ok(StatusCode::NO_CONTENT)
}

/// Parse a JSON array of document IDs in shorthand (`"uuid-string"`) or
/// longhand (`{ "documentId": "uuid-string" }`) format into `DocumentInstanceId`s.
fn parse_ids_from_list(value: &serde_json::Value) -> Result<Vec<DocumentInstanceId>, ApiError> {
    let arr = value.as_array().ok_or_else(|| {
        ApiError::UnprocessableEntity("connect/disconnect must be an array".into())
    })?;

    arr.iter()
        .map(|item| {
            let uuid_str = match item {
                serde_json::Value::String(s) => s.as_str(),
                serde_json::Value::Object(obj) => obj
                    .get("documentId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ApiError::UnprocessableEntity("documentId must be a string".into())
                    })?,
                _ => {
                    return Err(ApiError::UnprocessableEntity(
                        "each entry must be a UUID string or { documentId: '...' }".into(),
                    ));
                }
            };
            DocumentInstanceId::try_from(uuid_str).map_err(|_| {
                ApiError::UnprocessableEntity(format!("'{}' is not a valid UUID", uuid_str))
            })
        })
        .collect()
}
