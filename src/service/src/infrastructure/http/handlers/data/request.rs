use std::collections::HashMap;

use luminair_common::{AttributeId, DocumentType};
use serde_json::Value as JsonValue;

use crate::domain::document::content::ContentValue;
use crate::domain::document::error::DocumentError;

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
