use std::collections::HashMap;

use luminair_common::{AttributeId, DocumentType};
use serde_json::Value as JsonValue;

use crate::application::commands::RelationOperation;
use crate::domain::document::DocumentInstanceId;
use crate::domain::document::content::ContentValue;
use crate::domain::document::error::DocumentError;
use crate::infrastructure::http::api::ApiError;

/// Parsed and split request payload, ready for command construction.
#[derive(Debug)]
pub struct SplitPayload {
    pub field_payload: serde_json::Map<String, serde_json::Value>,
    pub relation_payload: serde_json::Map<String, serde_json::Value>,
}

/// Extract the `data` envelope from a JSON body and split its keys into
/// field values and relation operations based on the document type schema.
pub fn extract_and_split_payload(
    payload: &serde_json::Value,
    document_type: &DocumentType,
) -> Result<SplitPayload, ApiError> {
    let root_obj = payload.as_object().ok_or(ApiError::UnprocessableEntity(
        "body must be a JSON object".into(),
    ))?;
    let data_value = root_obj.get("data").ok_or(ApiError::UnprocessableEntity(
        "missing 'data' node in request body".into(),
    ))?;
    let data_obj = data_value.as_object().ok_or(ApiError::UnprocessableEntity(
        "payload must be a JSON object".into(),
    ))?;

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
                "Unknown field or relation: {}", k
            )));
        }
    }

    Ok(SplitPayload { field_payload, relation_payload })
}

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

pub fn parse_relation_operations(
    payload: &serde_json::Value,
) -> Result<HashMap<AttributeId, RelationOperation>, ApiError> {
    let data_obj = payload.as_object().ok_or(ApiError::UnprocessableEntity(
        "body must be a JSON object".into(),
    ))?;

    let mut operations = HashMap::new();

    for (field_name, field_value) in data_obj {
        let attr_id = AttributeId::try_new(field_name).map_err(|_| {
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

    Ok(operations)
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

#[cfg(test)]
mod tests {
    use super::*;
    use luminair_common::entities::{
        DocumentField, DocumentKind, DocumentRelation, DocumentTitle, DocumentTypeInfo, RelationType,
    };
    use luminair_common::{AttributeId, DocumentType, DocumentTypeId};
    use serde_json::json;
    use std::collections::HashSet;

    fn mock_document_type() -> DocumentType {
        DocumentType {
            id: DocumentTypeId::try_new("article").unwrap(),
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new("Article").unwrap(),
                singular_name: DocumentTypeId::try_new("article").unwrap(),
                plural_name: DocumentTypeId::try_new("articles").unwrap(),
                description: None,
            },
            options: None,
            fields: HashSet::from([
                DocumentField {
                    id: AttributeId::try_new("title").unwrap(),
                    field_type: luminair_common::entities::FieldType::Text,
                    constraints: HashSet::new(),
                    required: true,
                    unique: false,
                },
            ]),
            relations: HashSet::from([
                DocumentRelation {
                    id: AttributeId::try_new("author").unwrap(),
                    target: DocumentTypeId::try_new("author").unwrap(),
                    relation_type: RelationType::HasOne,
                },
            ]),
        }
    }

    #[test]
    fn test_extract_and_split_payload_success() {
        let dt = mock_document_type();
        let payload = json!({
            "data": {
                "title": "My Article",
                "author": {
                    "connect": ["9c00b05b-800e-436f-8705-d14bfb2875b4"]
                }
            }
        });

        let split = extract_and_split_payload(&payload, &dt).unwrap();
        assert_eq!(split.field_payload.get("title").unwrap().as_str().unwrap(), "My Article");
        assert!(split.relation_payload.contains_key("author"));
    }

    #[test]
    fn test_extract_and_split_payload_missing_data() {
        let dt = mock_document_type();
        let payload = json!({
            "title": "My Article"
        });

        let res = extract_and_split_payload(&payload, &dt);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("missing 'data'"));
    }

    #[test]
    fn test_extract_and_split_payload_unknown_field() {
        let dt = mock_document_type();
        let payload = json!({
            "data": {
                "ghost": "boo"
            }
        });

        let res = extract_and_split_payload(&payload, &dt);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Unknown field or relation"));
    }
}

