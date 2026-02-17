use chrono::{DateTime, Utc};
use luminair_common::{
    CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType, ID_FIELD_NAME,
    PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, UPDATED_BY_FIELD_NAME,
    UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
    entities::{AttributeType, DocumentField},
};
use sqlx::{
    Row,
    postgres::PgRow,
    types::{Uuid, uuid},
};

use crate::domain::{
    document::{
        DatabaseRowId, DocumentContent, DocumentInstance, DocumentInstanceId,
        content::{ContentValue, DomainValue},
        lifecycle::{AuditTrail, PublicationState, UserId},
    },
    repository::RepositoryError,
};

pub fn row_to_document(
    row: &sqlx::postgres::PgRow,
    schema: &DocumentType,
) -> Result<DocumentInstance, RepositoryError> {
    use chrono::{DateTime, Utc};
    use sqlx::Row;

    // Extract system fields
    let id: i64 = row
        .try_get(ID_FIELD_NAME)
        .map_err(|e| RepositoryError::DatabaseError(format!("Failed to parse id: {}", e)))?;
    let id = DatabaseRowId(id);

    let document_id: Uuid = row
        .try_get(DOCUMENT_ID_FIELD_NAME)
        .map_err(|e| RepositoryError::DatabaseError(format!("Failed to parse id: {}", e)))?;
    let document_id = DocumentInstanceId(document_id);

    // Extract field values
    let mut fields = std::collections::HashMap::new();
    for (field_id, field) in schema.fields.iter() {
        let normalized_name = field_id.normalized();
        let column_name: &str = normalized_name.as_ref();

        let value = parse_field_value(row, field, column_name)?;

        fields.insert(field_id.normalized().to_string(), value);
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
    })
}

pub fn parse_field_value(
    row: &PgRow,
    field: &DocumentField,
    column_name: &str,
) -> Result<ContentValue, RepositoryError> {
    let value = match field.attribute_type {
        AttributeType::Text => {
            let text_value: Option<String> = row.try_get(column_name).map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to parse text field {}: {}",
                    column_name, e
                ))
            })?;
            match text_value {
                Some(v) => ContentValue::Scalar(DomainValue::Text(v)),
                None => ContentValue::Scalar(DomainValue::Null),
            }
        }
        AttributeType::Integer => {
            let int_value: Option<i64> = row.try_get(column_name).map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to parse integer field {}: {}",
                    column_name, e
                ))
            })?;
            match int_value {
                Some(v) => ContentValue::Scalar(DomainValue::Integer(v)),
                None => ContentValue::Scalar(DomainValue::Null),
            }
        }
        AttributeType::Decimal => {
            let dec_value: Option<f64> = row.try_get(column_name).map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to parse decimal field {}: {}",
                    column_name, e
                ))
            })?;
            match dec_value {
                Some(v) => ContentValue::Scalar(DomainValue::Decimal(v)),
                None => ContentValue::Scalar(DomainValue::Null),
            }
        }
        AttributeType::Boolean => {
            let bool_value: Option<bool> = row.try_get(column_name).map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to parse boolean field {}: {}",
                    column_name, e
                ))
            })?;
            match bool_value {
                Some(v) => ContentValue::Scalar(DomainValue::Boolean(v)),
                None => ContentValue::Scalar(DomainValue::Null),
            }
        }
        AttributeType::Date => {
            let date_value: Option<chrono::NaiveDate> = row.try_get(column_name).map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to parse date field {}: {}",
                    column_name, e
                ))
            })?;
            match date_value {
                Some(v) => ContentValue::Scalar(DomainValue::Date(v)),
                None => ContentValue::Scalar(DomainValue::Null),
            }
        }
        AttributeType::DateTime => {
            let datetime_value: Option<DateTime<Utc>> = row.try_get(column_name).map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to parse datetime field {}: {}",
                    column_name, e
                ))
            })?;
            match datetime_value {
                Some(v) => ContentValue::Scalar(DomainValue::DateTime(v)),
                None => ContentValue::Scalar(DomainValue::Null),
            }
        }
        AttributeType::Uid | AttributeType::Uuid => {
            let uuid_value: Option<uuid::Uuid> = row.try_get(column_name).map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to parse uuid field {}: {}",
                    column_name, e
                ))
            })?;
            match uuid_value {
                Some(v) => ContentValue::Scalar(DomainValue::Uuid(v)),
                None => ContentValue::Scalar(DomainValue::Null),
            }
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
        created_by: created_by.map(UserId),
        updated_at,
        updated_by: updated_by.map(UserId),
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
                published_by: published_by.map(UserId),
            },
            None => PublicationState::Draft { revision: 1 },
        }
    } else {
        PublicationState::Published {
            revision: 1,
            published_at: created_at,
            published_by: None,
        }
    })
}
