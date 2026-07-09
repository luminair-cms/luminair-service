use std::collections::HashMap;

use luminair_common::{AttributeId, DocumentType};

use crate::application::commands::RelationOperation;
use crate::domain::document::DocumentInstanceId;
use crate::domain::document::content::ContentValue;
use crate::domain::document::error::DocumentError;
use crate::infrastructure::http::api::ApiError;

/// Classified JSON fields and relations, ready for parsing into domain types/operations.
#[derive(Debug)]
pub struct ClassifiedDocumentData {
    pub fields: HashMap<AttributeId, serde_json::Value>,
    pub relations: HashMap<AttributeId, serde_json::Value>,
}

/// Extract the `data` envelope from a JSON body.
pub fn extract_data_envelope(
    payload: &serde_json::Value,
) -> Result<&serde_json::Map<String, serde_json::Value>, ApiError> {
    let root_obj = payload
        .as_object()
        .ok_or_else(|| ApiError::UnprocessableEntity("body must be a JSON object".into()))?;
    let data_value = root_obj.get("data").ok_or_else(|| {
        ApiError::UnprocessableEntity("missing 'data' node in request body".into())
    })?;
    let data_obj = data_value
        .as_object()
        .ok_or_else(|| ApiError::UnprocessableEntity("payload must be a JSON object".into()))?;
    Ok(data_obj)
}

/// Classify the document data keys into field values and relation operations
/// based on the document type schema.
pub fn classify_document_data(
    data_obj: &serde_json::Map<String, serde_json::Value>,
    document_type: &DocumentType,
) -> Result<ClassifiedDocumentData, ApiError> {
    let mut fields = HashMap::new();
    let mut relations = HashMap::new();

    for (k, v) in data_obj {
        let attr_id = AttributeId::try_new(k)
            .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid field name: {}", k)))?;

        if document_type.fields.contains(&attr_id) {
            fields.insert(attr_id, v.clone());
        } else if document_type.relations.contains(&attr_id) {
            relations.insert(attr_id, v.clone());
        } else {
            return Err(ApiError::UnprocessableEntity(format!(
                "Unknown field or relation: {}",
                k
            )));
        }
    }

    Ok(ClassifiedDocumentData { fields, relations })
}

/// Parse and validate a JSON request map into a field map.
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
/// - Fields not declared on the document type
/// - Type mismatches or constraint violations (via the codec)
/// - Required fields explicitly set to `null`
pub fn build_fields_from_map(
    document_type: &DocumentType,
    fields_map: &HashMap<AttributeId, serde_json::Value>,
) -> Result<HashMap<AttributeId, ContentValue>, DocumentError> {
    let mut fields = HashMap::with_capacity(fields_map.len());

    for (attribute_id, field_value) in fields_map {
        let field_def = document_type.fields.get(attribute_id).ok_or_else(|| {
            DocumentError::InvalidFieldValue {
                field: attribute_id.as_ref().to_string(),
                reason: "unknown field for this document type".into(),
            }
        })?;

        fields.insert(
            attribute_id.clone(),
            ContentValue::from_json(field_value, field_def)?,
        );
    }

    Ok(fields)
}

pub fn parse_relation_operations(
    relations_map: &HashMap<AttributeId, serde_json::Value>,
) -> Result<HashMap<AttributeId, RelationOperation>, ApiError> {
    let mut operations = HashMap::new();

    for (attr_id, field_value) in relations_map {
        let field_obj = field_value.as_object().ok_or_else(|| {
            ApiError::UnprocessableEntity(format!("Field '{}' must be an object", attr_id.as_ref()))
        })?;

        if field_obj.contains_key("set") {
            return Err(ApiError::UnprocessableEntity(format!(
                "Relation field '{}': 'set' operation is not yet supported in the MVP",
                attr_id.as_ref()
            )));
        }

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
            attr_id.clone(),
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
        DocumentField, DocumentKind, DocumentRelation, DocumentTitle, DocumentTypeInfo,
        RelationType,
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
            fields: HashSet::from([DocumentField {
                id: AttributeId::try_new("title").unwrap(),
                field_type: luminair_common::entities::FieldType::Text,
                constraints: HashSet::new(),
                required: true,
                unique: false,
            }]),
            relations: HashSet::from([DocumentRelation {
                id: AttributeId::try_new("author").unwrap(),
                target: DocumentTypeId::try_new("author").unwrap(),
                relation_type: RelationType::HasOne,
            }]),
        }
    }

    #[test]
    fn test_extract_data_envelope_success() {
        let payload = json!({
            "data": {
                "title": "My Article"
            }
        });
        let data = extract_data_envelope(&payload).unwrap();
        assert_eq!(data.get("title").unwrap().as_str().unwrap(), "My Article");
    }

    #[test]
    fn test_extract_data_envelope_missing() {
        let payload = json!({
            "title": "My Article"
        });
        let res = extract_data_envelope(&payload);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("missing 'data'"));
    }

    #[test]
    fn test_classify_document_data_success() {
        let dt = mock_document_type();
        let payload = json!({
            "title": "My Article",
            "author": {
                "connect": ["9c00b05b-800e-436f-8705-d14bfb2875b4"]
            }
        });
        let data_map = payload.as_object().unwrap();

        let classified = classify_document_data(data_map, &dt).unwrap();
        assert_eq!(
            classified
                .fields
                .get(&AttributeId::try_new("title").unwrap())
                .unwrap()
                .as_str()
                .unwrap(),
            "My Article"
        );
        assert!(
            classified
                .relations
                .contains_key(&AttributeId::try_new("author").unwrap())
        );
    }

    #[test]
    fn test_classify_document_data_unknown_field() {
        let dt = mock_document_type();
        let payload = json!({
            "ghost": "boo"
        });
        let data_map = payload.as_object().unwrap();

        let res = classify_document_data(data_map, &dt);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("Unknown field or relation")
        );
    }

    #[test]
    fn test_parse_relation_operations_rejects_set() {
        let payload = json!({
            "author": {
                "set": ["9c00b05b-800e-436f-8705-d14bfb2875b4"]
            }
        });
        let mut map = HashMap::new();
        map.insert(
            AttributeId::try_new("author").unwrap(),
            payload.get("author").unwrap().clone(),
        );

        let res = parse_relation_operations(&map);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("set' operation is not yet supported")
        );
    }
}
