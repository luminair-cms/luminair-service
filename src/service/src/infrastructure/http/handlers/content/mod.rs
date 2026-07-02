use crate::application::AppState;
use crate::application::service::DocumentsService;
use crate::domain::document::DocumentInstanceId;
use crate::domain::query::{DocumentInstanceQuery, DocumentStatus};
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
use crate::application::commands::{CreateDocumentCommand, DeleteDocumentCommand, FindByIdCommand, FindDocumentsCommand, PublishDocumentCommand};
use luminair_common::AttributeId;

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
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
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

    let query_str = raw_query.unwrap_or_default();
    let (_, populate_filters, _) = params::parse_filters_and_sorts(&query_str, document_type, &state)?;

    let query = DocumentInstanceQuery::new().with_status(status);

    let cmd = FindByIdCommand {
        document_type,
        document_instance_id,
        populate: populate_attributes,
        populate_filters,
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
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<ManyDocumentsResponse>, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;

    let query_str = raw_query.unwrap_or_default();
    let (filter, populate_filters, sorts) = params::parse_filters_and_sorts(&query_str, document_type, &state)?;

    // Extract pagination params with defaults
    let (page, page_size) = params
        .pagination_or_default();

    let mut query = DocumentInstanceQuery::new()
        .paginate(page, page_size)
        .with_status(parse_status(&params.status)?)
        .with_filter(filter);

    query.sort = sorts;

    let cmd = FindDocumentsCommand {
        document_type,
        populate: parse_populate(params.populate, document_type)?,
        populate_filters,
        query,
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

    let root_obj = payload.as_object().ok_or(ApiError::UnprocessableEntity(
        "body must be a JSON object".into(),
    ))?;
    let data_value = root_obj.get("data").ok_or(ApiError::UnprocessableEntity(
        "missing 'data' node in request body".into(),
    ))?;

    let data_obj = data_value.as_object().ok_or(ApiError::UnprocessableEntity(
        "payload must be a JSON object".into(),
    ))?;

    // Split payload fields into normal content fields vs relation operations
    let mut field_payload = serde_json::Map::new();
    let mut relation_payload = serde_json::Map::new();

    for (k, v) in data_obj {
        let attr_id = AttributeId::try_new(k).map_err(|_| {
            ApiError::UnprocessableEntity(format!("Invalid field name: {}", k))
        })?;

        if document_type.relations.contains(&attr_id) {
            relation_payload.insert(k.clone(), v.clone());
        } else if document_type.fields.contains(&attr_id) {
            field_payload.insert(k.clone(), v.clone());
        } else {
            return Err(ApiError::UnprocessableEntity(format!(
                "Unknown field or relation: {}",
                k
            )));
        }
    }

    // 1. Create document using content fields
    let create_cmd = request::parse_create_command(
        document_type,
        &serde_json::Value::Object(field_payload),
        None,
    )?;

    let created_document_id = state
        .documents_service()
        .create(create_cmd)
        .await
        .map_err(|err| ApiError::from(err))?;

    // 2. Connect relations if any are specified in the payload
    if !relation_payload.is_empty() {
        let modify_cmd = request::parse_modify_relations_command(
            document_type,
            created_document_id,
            &serde_json::Value::Object(relation_payload),
        )?;
        state.documents_service().modify_relations(modify_cmd).await?;
    }

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

    let root_obj = payload.as_object().ok_or(ApiError::UnprocessableEntity(
        "body must be a JSON object".into(),
    ))?;
    let data_value = root_obj.get("data").ok_or(ApiError::UnprocessableEntity(
        "missing 'data' node in request body".into(),
    ))?;

    let data_obj = data_value.as_object().ok_or(ApiError::UnprocessableEntity(
        "payload must be a JSON object".into(),
    ))?;

    // Split payload fields into normal content fields vs relation operations
    let mut field_payload = serde_json::Map::new();
    let mut relation_payload = serde_json::Map::new();

    for (k, v) in data_obj {
        let attr_id = AttributeId::try_new(k).map_err(|_| {
            ApiError::UnprocessableEntity(format!("Invalid field name: {}", k))
        })?;

        if document_type.relations.contains(&attr_id) {
            relation_payload.insert(k.clone(), v.clone());
        } else if document_type.fields.contains(&attr_id) {
            field_payload.insert(k.clone(), v.clone());
        } else {
            return Err(ApiError::UnprocessableEntity(format!(
                "Unknown field or relation: {}",
                k
            )));
        }
    }

    // 1. Apply field updates if present
    if !field_payload.is_empty() {
        let cmd = request::parse_update_command(
            document_type,
            document_instance_id,
            &serde_json::Value::Object(field_payload),
            None,
        )?;
        state.documents_service().update(cmd).await?;
    }

    // 2. Apply relation operations if present
    if !relation_payload.is_empty() {
        let cmd = request::parse_modify_relations_command(
            document_type,
            document_instance_id,
            &serde_json::Value::Object(relation_payload),
        )?;
        state.documents_service().modify_relations(cmd).await?;
    }

    // 3. Return the fully updated document state (with status: Draft to see the latest working copy)
    let query = DocumentInstanceQuery::new().with_status(DocumentStatus::Draft);
    let find_cmd = FindByIdCommand {
        document_type,
        document_instance_id,
        populate: None,
        populate_filters: None,
        query,
    };

    let updated_instance = state
        .documents_service()
        .find_by_id(find_cmd)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(ApiSuccess::new(
        StatusCode::OK,
        OneDocumentResponse::try_from(Some(updated_instance))
            .map_err(|_| ApiError::NotFound)?,
    ))
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
        OneDocumentResponse::try_from(Some(published_instance))
            .map_err(|_| ApiError::NotFound)?,
    ))
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
