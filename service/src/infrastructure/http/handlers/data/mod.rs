use crate::domain::AppState;
use crate::domain::application::{DocumentServices};
use crate::domain::document::{DocumentInstanceId};
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
use std::collections::HashSet;
use std::fmt::format;
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

    let document_response = state
        .documents_services()
        .find_by_id(document_type, populate_attributes, document_instance_id)
        .await
        .map_err(|err| ApiError::from(err))?
        .ok_or(ApiError::NotFound)?
        .into();

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

    // Build query using builder pattern - pagination guards are enforced by the query
    let query = DocumentInstanceQuery::new().paginate(page, page_size);

    let documents: Vec<response::DocumentInstanceResponse> = state
        .documents_services()
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
        .documents_services()
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
        .documents_services()
        .delete(document_type, instance_id)
        .await
        .map_err(|err| ApiError::from(err))?;

    Ok((StatusCode::NO_CONTENT, ()))
}
