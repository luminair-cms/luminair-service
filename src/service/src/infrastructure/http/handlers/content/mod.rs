use crate::application::AppState;
use crate::application::service::DocumentsService;
use crate::domain::document::DocumentInstanceId;
use crate::domain::query::DocumentInstanceQuery;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::content::params::{parse_populate, parse_status, resolve_document_type, QueryParams};
use crate::infrastructure::http::handlers::content::response::{
    ManyDocumentsResponse, OneDocumentResponse,
};
use crate::infrastructure::http::querystring::QueryString;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use crate::application::commands::{CreateDocumentCommand, DeleteDocumentCommand, FindByIdCommand, FindDocumentsCommand};

mod params;
mod request;
mod response;

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

    let document_type = resolve_document_type(&state, &api_type)?;
    let document_instance_id = DocumentInstanceId::try_from(&id)?;
    let populate_attributes = parse_populate(params.populate, document_type)?;
    let status = parse_status(&params.status)?;
    let query = DocumentInstanceQuery::new().with_status(status);

    let cmd = FindByIdCommand {
        document_type,
        document_instance_id,
        populate: populate_attributes,
        query
    };

    let document_instance = state
        .documents_service()
        .find_by_id(cmd)
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
    let document_type = resolve_document_type(&state, &api_type)?;

    // Extract pagination params with defaults
    let (page, page_size) = params
        .pagination_or_default();
    
    let cmd = FindDocumentsCommand {
        document_type,
        populate: parse_populate(params.populate, document_type)?,
        query: DocumentInstanceQuery::new()
            .paginate(page, page_size)
            .with_status(parse_status(&params.status)?),
    };

    let (documents, total) = state.documents_service()
        .find(cmd).await?;
    
    Ok(ApiSuccess::new(
        StatusCode::OK,
        ManyDocumentsResponse::new(documents, page, page_size, total)))
}

pub async fn create_new_document<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;

    // Validate and build fields from the payload using document type metadata
    let fields = request::build_fields_from_payload(document_type, &payload).map_err(
        |err: crate::domain::document::error::DocumentError| {
            ApiError::UnprocessableEntity(err.to_string())
        },
    )?;

    let cmd = CreateDocumentCommand {
        document_type,
        fields,
        user_id: None
    };

    let created_document_id = state
        .documents_service()
        .create(cmd)
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
    let document_type = resolve_document_type(&state, &api_type)?;
    let document_instance_id = DocumentInstanceId::try_from(&id)?;

    let cmd = DeleteDocumentCommand {
        document_type,
        document_instance_id
    };

    state
        .documents_service()
        .delete(cmd)
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
    let document_type = resolve_document_type(&state, &api_type)?;
    let document_instance_id = DocumentInstanceId::try_from(&id)?;

    let cmd = request::parse_modify_relations_command(document_type, document_instance_id, &payload)?;

    state
        .documents_service()
        .modify_relations(cmd)
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
