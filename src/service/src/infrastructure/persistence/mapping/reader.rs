use crate::domain::document::content::DocumentContent;
use crate::domain::{
    document::{
        DatabaseRowId, DocumentInstance, DocumentInstanceId,
        content::{ContentValue, DomainValue},
        lifecycle::{AuditTrail, PublicationState, UserId},
    },
    repository::RepositoryError,
};
use chrono::{DateTime, Utc};
use luminair_common::{
    AttributeId, CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType,
    PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, SNAPSHOT_ID_FIELD_NAME,
    UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
    entities::{DocumentField, FieldType},
};
use rust_decimal::Decimal;
use sqlx::postgres::PgValueRef;
use sqlx::{
    Postgres, Row, Type, ValueRef,
    decode::Decode,
    postgres::PgRow,
    types::{Json, Uuid},
};
use std::collections::HashMap;
use std::str::FromStr;

pub fn row_to_document(
    row: &PgRow,
    schema: &DocumentType,
) -> Result<DocumentInstance, RepositoryError> {
    use chrono::{DateTime, Utc};
    use sqlx::Row;

    // Extract system fields
    let id = match row.try_get::<i64, _>(SNAPSHOT_ID_FIELD_NAME) {
        Ok(sid) => DatabaseRowId(sid),
        Err(_) => DatabaseRowId(0),
    };

    let document_id: Uuid = row
        .try_get(DOCUMENT_ID_FIELD_NAME)
        .map_err(|e| RepositoryError::DatabaseError(format!("Failed to parse id: {}", e)))?;
    let document_id = DocumentInstanceId(document_id);

    // Extract field values
    let mut fields = HashMap::new();
    for field in schema.fields.iter() {
        let normalized_name = field.id.normalized();
        let column_name: &str = normalized_name.as_ref();

        let value = parse_field_value(row, field, column_name)?;

        fields.insert(AttributeId::from_str(column_name).unwrap(), value);
    }

    let created_at: DateTime<Utc> = row.try_get(CREATED_FIELD_NAME).map_err(|e| {
        RepositoryError::DatabaseError(format!("Failed to parse created_at: {}", e))
    })?;

    let publication_state = parse_publication_state(row, schema, created_at)?;
    let audit = parse_audit_trail(row, created_at)?;

    let content = DocumentContent {
        fields,
        publication_state,
    };

    Ok(DocumentInstance {
        id,
        document_id,
        content,
        audit,
        relations: HashMap::new(),
    })
}

fn decode_value<'r, T>(value: PgValueRef<'r>) -> Result<T, RepositoryError>
where
    T: Decode<'r, Postgres> + Type<Postgres>,
{
    T::decode(value)
        .map_err(|e| RepositoryError::DatabaseError(format!("Failed to decode value: {}", e)))
}

pub fn parse_field_value(
    row: &PgRow,
    field: &DocumentField,
    column_name: &str,
) -> Result<ContentValue, RepositoryError> {
    let value_ref = row.try_get_raw(column_name).map_err(|e| {
        RepositoryError::DatabaseError(format!("Failed to parse field {}: {}", column_name, e))
    })?;

    if value_ref.is_null() {
        return Ok(ContentValue::Null);
    }

    // TODO: generalize this: DomainValue is depend on FieldType, both can precise param of row.try_get

    let value = match field.field_type {
        FieldType::Text => {
            let value: String = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Text(value))
        }
        FieldType::LocalizedText => {
            let value: Json<HashMap<String, String>> = decode_value(value_ref)?;
            ContentValue::LocalizedText(value.0)
        }
        // TODO: use different types for different integer sizes
        FieldType::Integer(_) => {
            let value: i64 = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Integer(value))
        }
        FieldType::Decimal { .. } => {
            let value: Decimal = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Decimal(value))
        }
        FieldType::Boolean => {
            let value: bool = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Boolean(value))
        }
        FieldType::Date => {
            let value: chrono::NaiveDate = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Date(value))
        }
        FieldType::DateTime => {
            let value: DateTime<Utc> = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::DateTime(value))
        }
        // Uid is a human-readable slug stored as a text column — not a UUID column.
        // This mirrors the from_json codec: FieldType::Uid → DomainValue::Text.
        FieldType::Uid => {
            let value: String = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Text(value))
        }
        FieldType::Uuid => {
            let value: Uuid = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Uuid(value))
        }
        FieldType::Json => {
            let value: Json<HashMap<String, String>> = decode_value(value_ref)?;
            ContentValue::Scalar(DomainValue::Json(value.0))
        }
    };
    Ok(value)
}

fn parse_audit_trail(
    row: &PgRow,
    created_at: DateTime<Utc>,
) -> Result<AuditTrail, RepositoryError> {
    let updated_at: DateTime<Utc> = row.try_get(UPDATED_FIELD_NAME).map_err(|e| {
        RepositoryError::DatabaseError(format!("Failed to parse updated_at: {}", e))
    })?;

    let created_by: Option<String> = row.try_get(CREATED_BY_FIELD_NAME).map_err(|e| {
        RepositoryError::DatabaseError(format!("Failed to parse created_by: {}", e))
    })?;

    let updated_by: Option<String> = row.try_get(UPDATED_BY_FIELD_NAME).map_err(|e| {
        RepositoryError::DatabaseError(format!("Failed to parse updated_by: {}", e))
    })?;

    let version: i32 = row
        .try_get(VERSION_FIELD_NAME)
        .map_err(|e| RepositoryError::DatabaseError(format!("Failed to parse version: {}", e)))?;

    let audit = AuditTrail {
        created_at,
        // UserId::try_new trims and rejects empty strings. DB values that are
        // somehow empty are treated as missing rather than panicking.
        created_by: created_by.and_then(|s| UserId::try_new(s).ok()),
        updated_at,
        updated_by: updated_by.and_then(|s| UserId::try_new(s).ok()),
        version,
    };
    Ok(audit)
}

// Parse publication state if the schema supports draft_and_publish
fn parse_publication_state(
    row: &PgRow,
    schema: &DocumentType,
    created_at: DateTime<Utc>,
) -> Result<PublicationState, RepositoryError> {
    Ok(if schema.has_draft_and_publish() {
        let published_at: Option<DateTime<Utc>> =
            row.try_get(PUBLISHED_FIELD_NAME).map_err(|e| {
                RepositoryError::DatabaseError(format!("Failed to parse published_at: {}", e))
            })?;
        let published_by: Option<String> = row.try_get(PUBLISHED_BY_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse updated_by: {}", e))
        })?;
        let revision: i32 = row.try_get(REVISION_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse revision: {}", e))
        })?;

        match published_at {
            Some(pub_at) => PublicationState::Published {
                revision,
                published_at: pub_at,
                published_by: published_by.and_then(|s| UserId::try_new(s).ok()),
            },
            None => PublicationState::Draft { revision },
        }
    } else {
        PublicationState::Published {
            revision: 1,
            published_at: created_at,
            published_by: None,
        }
    })
}
