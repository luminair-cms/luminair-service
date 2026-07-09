use crate::application::AppState;
use crate::application::commands::{
    CreateDocumentWithRelationsCommand, DeleteDocumentCommand, FindByIdCommand,
    FindDocumentsCommand, PublishDocumentCommand, UpdateDocumentWithRelationsCommand,
};
use crate::application::service::DocumentsService;
use crate::domain::document::DocumentInstanceId;
use crate::domain::query::DocumentInstanceQuery;
use crate::infrastructure::http::api::{ApiError, ApiSuccess};
use crate::infrastructure::http::handlers::content::response::{
    ManyDocumentsResponse, OneDocumentResponse,
};
use crate::infrastructure::http::querystring::QueryMap;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use luminair_common::{DocumentType, DocumentTypeApiId};
use std::str::FromStr;

mod params;
mod request;
mod response;

/// Resolve a `{api_type}` path segment to a registered [`DocumentType`].
fn resolve_document_type<S: AppState>(
    state: &S,
    api_type: &str,
) -> Result<&'static DocumentType, ApiError> {
    let api_id = DocumentTypeApiId::from_str(api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    state
        .document_types()
        .lookup(&api_id)
        .ok_or_else(|| ApiError::NotFound(format!("Document type '{}' not found", api_type)))
}

pub async fn find_document_by_id<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
    QueryMap(query_map): QueryMap,
) -> Result<ApiSuccess<OneDocumentResponse>, ApiError> {
    if query_map.contains_key("pagination") {
        return Err(ApiError::UnprocessableEntity(
            "Pagination param isn't eligible for find_by_id query".to_string(),
        ));
    }
    if query_map.contains_key("sort") {
        return Err(ApiError::UnprocessableEntity(
            "Sort param isn't eligible for find_by_id query".to_string(),
        ));
    }

    let document_type = resolve_document_type(&state, &api_type)?;
    let document_instance_id = DocumentInstanceId::try_from(&id)?;
    let q = params::parse_query(
        &query_map,
        document_type,
        state.document_types(),
        &state.pagination_settings(),
    )?;

    let query = DocumentInstanceQuery::new().with_status(q.status);

    let cmd = FindByIdCommand {
        document_type,
        document_instance_id,
        populate: q.populate,
        populate_filters: q.populate_filters,
        query,
    };

    let document_instance = state.documents_service().find_by_id(cmd).await?;

    OneDocumentResponse::from_optional(document_instance)
        .map(|response| ApiSuccess::new(StatusCode::OK, response))
        .ok_or_else(|| ApiError::NotFound(format!("Document instance with ID '{}' not found", id)))
}

pub async fn find_all_documents<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
    QueryMap(query_map): QueryMap,
) -> Result<ApiSuccess<ManyDocumentsResponse>, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;
    let q = params::parse_query(
        &query_map,
        document_type,
        state.document_types(),
        &state.pagination_settings(),
    )?;

    let (page, page_size) = q.pagination;
    let mut query = DocumentInstanceQuery::new()
        .paginate(page, page_size)
        .with_status(q.status)
        .with_filter(q.filter);

    query.sort = q.sorts;

    let cmd = FindDocumentsCommand {
        document_type,
        populate: q.populate,
        populate_filters: q.populate_filters,
        query,
    };

    let (documents, total) = state.documents_service().find(cmd).await?;

    Ok(ApiSuccess::new(
        StatusCode::OK,
        ManyDocumentsResponse::new(documents, page, page_size, total),
    ))
}

pub async fn create_new_document<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<(StatusCode, axum::http::HeaderMap), ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;
    let data_obj = request::extract_data_envelope(&payload)?;
    let classified = request::classify_document_data(data_obj, document_type)?;

    let fields = request::build_fields_from_map(document_type, &classified.fields)
        .map_err(|e| ApiError::UnprocessableEntity(e.to_string()))?;
    let relation_operations = request::parse_relation_operations(&classified.relations)?;

    let cmd = CreateDocumentWithRelationsCommand {
        document_type,
        fields,
        relation_operations,
        user_id: None,
    };

    let created_document_id = state.documents_service().create_with_relations(cmd).await?;

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

/// Handle updating document fields and/or modifying relations in a single PUT request.
///
/// Accepts a flat JSON payload or a nested `{ "data": { ... } }` payload.
pub async fn update_document_handler<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
    Json(payload): Json<serde_json::Value>,
) -> Result<ApiSuccess<OneDocumentResponse>, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;
    let document_instance_id = DocumentInstanceId::try_from(&id)?;

    let data_obj = request::extract_data_envelope(&payload)?;
    let classified = request::classify_document_data(data_obj, document_type)?;

    let fields = request::build_fields_from_map(document_type, &classified.fields)
        .map_err(|e| ApiError::UnprocessableEntity(e.to_string()))?;
    let relation_operations = request::parse_relation_operations(&classified.relations)?;

    let cmd = UpdateDocumentWithRelationsCommand {
        document_type,
        document_id: document_instance_id,
        fields,
        relation_operations,
        user_id: None,
    };

    let updated_instance = state.documents_service().update_with_relations(cmd).await?;

    Ok(ApiSuccess::new(
        StatusCode::OK,
        OneDocumentResponse::from_optional(Some(updated_instance)).ok_or_else(|| {
            ApiError::NotFound("Document instance not found after update".to_string())
        })?,
    ))
}

pub async fn delete_existing_document<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;
    let document_instance_id = DocumentInstanceId::try_from(&id)?;

    let cmd = DeleteDocumentCommand {
        document_type,
        document_instance_id,
    };

    state.documents_service().delete(cmd).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Handle publishing a draft document.
pub async fn publish_document<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
) -> Result<ApiSuccess<OneDocumentResponse>, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;
    let document_instance_id = DocumentInstanceId::try_from(&id)?;

    let cmd = PublishDocumentCommand {
        document_type,
        document_id: document_instance_id,
        user_id: None,
    };

    let published_instance = state
        .documents_service()
        .publish(cmd)
        .await
        .map_err(ApiError::from)?;

    Ok(ApiSuccess::new(
        StatusCode::OK,
        OneDocumentResponse::from_optional(Some(published_instance)).ok_or_else(|| {
            ApiError::NotFound("Document instance not found after publish".to_string())
        })?,
    ))
}
