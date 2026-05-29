use std::collections::HashMap;

use luminair_common::{AttributeId, DocumentType};
use serde_json::Value as JsonValue;

use crate::application::commands::{CreateDocumentCommand, ModifyRelationsCommand, RelationOperation, UpdateDocumentCommand};
use crate::domain::document::DocumentInstanceId;
use crate::domain::document::content::ContentValue;
use crate::domain::document::error::DocumentError;
use crate::domain::document::lifecycle::UserId;
use crate::infrastructure::http::api::ApiError;
use crate::infrastructure::http::handlers::content::parse_ids_from_list;

/// Parse and validate a JSON request body into a field map.
///
/// Each key in the payload must be a valid [`AttributeId`] that exists on the
/// document type. Unknown fields are rejected. Fields that are declared
/// `required` and supplied as `null` are rejected.
///
/// All type conversion and [`FieldConstraint`] validation is delegated to
/// [`ContentValue::from_json`], which is the single canonical JSON → domain codec.
///
/// # Errors
///
/// Returns [`DocumentError`] for:
/// - A payload that is not a JSON object
/// - Field names that are not valid attribute identifiers
/// - Fields not declared on the document type
/// - Type mismatches or constraint violations (via the codec)
/// - Required fields explicitly set to `null`
pub fn build_fields_from_payload(
    document_type: &DocumentType,
    payload: &JsonValue,
) -> Result<HashMap<AttributeId, ContentValue>, DocumentError> {
    let payload_obj = payload
        .as_object()
        .ok_or_else(|| DocumentError::InvalidFieldValue {
            field: "<body>".into(),
            reason: "request body must be a JSON object".into(),
        })?;

    let mut fields = HashMap::with_capacity(payload_obj.len());

    for (field_name, field_value) in payload_obj {
        let attribute_id =
            AttributeId::try_new(field_name).map_err(|_| DocumentError::InvalidFieldValue {
                field: field_name.clone(),
                reason: "invalid field name".into(),
            })?;

        let field_def = document_type.fields.get(&attribute_id).ok_or_else(|| {
            DocumentError::InvalidFieldValue {
                field: field_name.clone(),
                reason: "unknown field for this document type".into(),
            }
        })?;

        fields.insert(
            attribute_id,
            ContentValue::from_json(field_value, field_def)?,
        );
    }

    Ok(fields)
}

pub fn parse_create_command(
    document_type: &'static DocumentType,
    payload: &serde_json::Value,
    user_id: Option<UserId>,
) -> Result<CreateDocumentCommand, ApiError> {
    let fields = build_fields_from_payload(document_type, &payload).map_err(
        |err: DocumentError| {
            ApiError::UnprocessableEntity(err.to_string())
        },
    )?;
    Ok(CreateDocumentCommand {
        document_type,
        fields,
        user_id,
    })
}

pub fn parse_update_command(
    document_type: &'static DocumentType,
    document_id: DocumentInstanceId,
    payload: &serde_json::Value,
    user_id: Option<UserId>,
) -> Result<UpdateDocumentCommand, ApiError> {
    let fields = build_fields_from_payload(document_type, &payload).map_err(
        |err: DocumentError| {
            ApiError::UnprocessableEntity(err.to_string())
        },
    )?;
    Ok(UpdateDocumentCommand {
        document_type,
        document_id,
        fields,
        user_id
    })
}

pub fn parse_modify_relations_command(
    document_type: &'static DocumentType,
    document_id: DocumentInstanceId,
    payload: &serde_json::Value,
) -> Result<ModifyRelationsCommand, ApiError> {
    let data_obj = payload.as_object().ok_or(ApiError::UnprocessableEntity(
        "body must be a JSON object".into(),
    ))?;

    let mut operations = HashMap::new();

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

        operations.insert(
            attr_id,
            RelationOperation::ConnectDisconnect {
                connect,
                disconnect,
            },
        );
    }

    Ok(ModifyRelationsCommand {
        document_type,
        document_id,
        operations
    })
}
