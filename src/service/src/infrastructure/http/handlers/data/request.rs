use anyhow::{anyhow, Context, Result};
use luminair_common::{AttributeId, DocumentType, entities::{FieldType, DocumentField}};
use serde_json::Value as JsonValue;
use crate::domain::document::content::{ContentValue, DomainValue};
use std::collections::HashMap;
use regex::Regex;
use rust_decimal::Decimal;

/// Validates and constructs fields from JSON payload using DocumentType metadata
pub fn build_fields_from_payload(
    document_type: &DocumentType,
    payload: &JsonValue,
) -> Result<HashMap<AttributeId, ContentValue>> {
    let mut fields = HashMap::new();

    let payload_obj = payload
        .as_object()
        .ok_or_else(|| anyhow!("Payload must be a JSON object"))?;

    // Validate and process each field from the payload
    for (field_name, field_value) in payload_obj.iter() {
        // Find the field definition in document type
        let attribute_id = AttributeId::try_new(field_name)
            .map_err(|_| anyhow!("Invalid field name: {}", field_name))?;

        let field_def = document_type
            .fields
            .get(&attribute_id)
            .ok_or_else(|| anyhow!("Unknown field: {}", field_name))?;

        // Validate required fields
        if field_def.required && field_value.is_null() {
            return Err(anyhow!("Field '{}' is required", field_name));
        }
        
        let content_value = convert_to_content_value(
            field_value,
            field_def,
            field_name,
        )?;
        
        fields.insert(attribute_id, content_value);
    }

    Ok(fields)
}

/// Converts a JSON value to DomainValue with type validation and constraint checking
fn convert_to_content_value(
    value: &JsonValue,
    field_def: &DocumentField,
    field_name: &str,
) -> Result<ContentValue> {
    if value.is_null() {
        return Ok(ContentValue::Null);
    }

    match field_def.field_type {
        FieldType::Text => {
                let text = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Field '{}' must be a string", field_name))?
                    .to_string();

                Ok(ContentValue::Scalar(DomainValue::Text(text)))
        }

        FieldType::LocalizedText => {
            let json_obj = value
                .as_object()
                .ok_or_else(|| anyhow!("Field '{}' must be a JSON object", field_name))?;
            let mut localized_map = HashMap::new();

            for (locale, val) in json_obj {
                let text = val
                    .as_str()
                    .ok_or_else(|| anyhow!("Value for locale '{}' in field '{}' must be a string", locale, field_name))?
                    .to_string();

                localized_map.insert(locale.clone(), text);
            }

            Ok(ContentValue::LocalizedText(localized_map))
        }

        // TODO: use different types for different integer sizes
        FieldType::Integer { .. } => {
            let int = value
                .as_i64()
                .ok_or_else(|| anyhow!("Field '{}' must be an integer", field_name))?;

            Ok(ContentValue::Scalar(DomainValue::Integer(int)))
        }
        
        FieldType::Decimal { scale, precision } => {
            use rust_decimal::prelude::*;
            let float_value = value.as_f64()
                .ok_or_else(|| anyhow!("Field '{}' must be a float", field_name))?;

            let mut decimal: Decimal = Decimal::from_f64_retain(float_value)
                .ok_or_else(|| anyhow!("Failed to convert float to Decimal"))?;
            decimal.rescale(scale);

            Ok(ContentValue::Scalar(DomainValue::Decimal(decimal)))
        }

        FieldType::Boolean => {
            let boolean = value
                .as_bool()
                .ok_or_else(|| anyhow!("Field '{}' must be a boolean", field_name))?;

            Ok(ContentValue::Scalar(DomainValue::Boolean(boolean)))
        }

        FieldType::Date => {
            let date_str = value
                .as_str()
                .ok_or_else(|| anyhow!("Field '{}' must be a string in YYYY-MM-DD format", field_name))?;

            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|_| anyhow!("Field '{}' has invalid date format", field_name))?;

            Ok(ContentValue::Scalar(DomainValue::Date(date)))
        }

        FieldType::DateTime => {
            let datetime_str = value
                .as_str()
                .ok_or_else(|| anyhow!("Field '{}' must be a string in RFC3339 format", field_name))?;

            let datetime = chrono::DateTime::parse_from_rfc3339(datetime_str)
                .map_err(|_| anyhow!("Field '{}' has invalid datetime format", field_name))?
                .with_timezone(&chrono::Utc);

            Ok(ContentValue::Scalar(    DomainValue::DateTime(datetime)))
        }

        FieldType::Uid => {
            let uid = value
                .as_str()
                .ok_or_else(|| anyhow!("Field '{}' must be a string", field_name))?
                .to_string();

            // Validate UID uniqueness constraint
            if field_def.unique {
                // TODO: Check against repository for existing values
            }

            Ok(ContentValue::Scalar(DomainValue::Text(uid)))
        }

        FieldType::Uuid => {
            let uuid_str = value
                .as_str()
                .ok_or_else(|| anyhow!("Field '{}' must be a UUID string", field_name))?;

            let uuid = sqlx::types::uuid::Uuid::parse_str(uuid_str)
                .map_err(|_| anyhow!("Field '{}' has invalid UUID format", field_name))?;
            
            // Validate UID uniqueness constraint
            if field_def.unique {
                // TODO: Check against repository for existing values
            }

            Ok(ContentValue::Scalar(DomainValue::Uuid(uuid)))
        }
        
        _ => Err(anyhow!("Unsupported field type: {:?}", field_def.field_type)),
    }
}
