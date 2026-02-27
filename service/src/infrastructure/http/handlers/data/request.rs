use anyhow::{anyhow, Result};
use luminair_common::{AttributeId, DocumentType, entities::{FieldType, DocumentField}};
use serde_json::Value as JsonValue;
use crate::domain::document::content::{ContentValue, DomainValue};
use std::collections::HashMap;
use regex::Regex;

/// Validates and constructs fields from JSON payload using DocumentType metadata
pub fn build_fields_from_payload(
    document_type: &DocumentType,
    payload: &JsonValue,
) -> Result<HashMap<String, ContentValue>> {
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
        
        fields.insert(field_name.clone(), content_value);
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
        FieldType::Text { localized } => {
            if localized {
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
            } else {
                let text = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Field '{}' must be a string", field_name))?
                    .to_string();

                // Validate constraints
                validate_text_constraints(&text, field_def, field_name)?;

                Ok(ContentValue::Scalar(DomainValue::Text(text)))
            }
        }

        FieldType::Integer => {
            let int = value
                .as_i64()
                .ok_or_else(|| anyhow!("Field '{}' must be an integer", field_name))?;

            Ok(ContentValue::Scalar(DomainValue::Integer(int)))
        }

        // TODO: consider using Decimal type from rust_decimal crate for better precision
        FieldType::Decimal => {
            let decimal = value
                .as_f64()
                .ok_or_else(|| anyhow!("Field '{}' must be a decimal number", field_name))?;

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

            validate_text_constraints(&uid, field_def, field_name)?;

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

/// Validates text field constraints (length, pattern)
fn validate_text_constraints(
    text: &str,
    field_def: &DocumentField,
    field_name: &str,
) -> Result<()> {
    if let Some(constraints) = &field_def.constraints {
        // Check minimal length
        if let Some(min_len) = constraints.minimal_length {
            if text.len() < min_len {
                return Err(anyhow!(
                    "Field '{}' must be at least {} characters long",
                    field_name,
                    min_len
                ));
            }
        }

        // Check maximal length
        if let Some(max_len) = constraints.maximal_length {
            if text.len() > max_len {
                return Err(anyhow!(
                    "Field '{}' must be at most {} characters long",
                    field_name,
                    max_len
                ));
            }
        }

        // Check pattern
        if let Some(pattern) = &constraints.pattern {
            let regex = Regex::new(pattern)
                .map_err(|_| anyhow!("Invalid regex pattern in field definition: {}", field_name))?;

            if !regex.is_match(text) {
                return Err(anyhow!(
                    "Field '{}' does not match required pattern",
                    field_name
                ));
            }
        }
    }

    Ok(())
}
