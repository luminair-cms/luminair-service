use std::borrow::Cow;

use futures::future::ok;
use luminair_common::{
    CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType, DocumentTypeId,
    DocumentTypesRegistry, PUBLISHED_FIELD_NAME, UPDATED_FIELD_NAME, database::Database,
    entities::AttributeType,
};
use sqlx::types::uuid;

use crate::{
    domain::{
        document::{
            DocumentContent, DocumentInstance, DocumentInstanceId,
            content::{AuditTrail, ContentValue, DomainValue, PublicationState},
        },
        repository::{DocumentInstanceRepository, RepositoryError},
    },
    infrastructure::persistence::{
        query::{Condition, ConditionValue, QueryBuilder},
        schema::{Column, ColumnRef, Table},
    },
};

#[derive(Clone)]
pub struct PostgresDocumentRepository {
    schema_registry: &'static dyn DocumentTypesRegistry,
    database: &'static Database,
}

/// Common columns

const DOCUMENT_ID_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: DOCUMENT_ID_FIELD_NAME,
};
const CREATED_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: CREATED_FIELD_NAME,
};
const UPDATED_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: UPDATED_FIELD_NAME,
};
const PUBLISHED_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: PUBLISHED_FIELD_NAME,
};

impl DocumentInstanceRepository for PostgresDocumentRepository {
    async fn find(
        &self,
        query: crate::domain::repository::query::DocumentInstanceQuery,
    ) -> Result<Vec<crate::domain::document::DocumentInstance>, RepositoryError> {
        let document_type_id = &query.document_type_id;
        let schema = self
            .schema_registry
            .get(document_type_id)
            .ok_or(RepositoryError::NotFound)?;

        let table = Table::from(document_type_id);
        let mut columns: Vec<ColumnRef<'_>> = vec![
            Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            Cow::Borrowed(&CREATED_COLUMN),
            Cow::Borrowed(&UPDATED_COLUMN),
        ];

        if schema.has_draft_and_publish() {
            columns.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        }

        for (id, field) in schema.fields.iter() {
            let column = Column {
                qualifier: "m",
                name: id.normalized().as_ref(),
            };
            columns.push(Cow::Owned(column));
        }

        let query_builder = QueryBuilder::from(table).select(columns);
        let (sql, params) = query_builder.build();
        println!("Generated SQL: {}", sql);

        let mut query_object = sqlx::query(&sql);
        for param in params {
            query_object = param.bind_to_query(query_object);
        }

        let rows = query_object
            .fetch_all(self.pool.as_ref())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let documents: Result<Vec<_>, _> = rows
            .into_iter()
            .map(|row| self.row_to_document(&row, &schema))
            .collect();

        documents
    }

    async fn find_by_id(
        &self,
        document_type_id: DocumentTypeId,
        id: DocumentInstanceId,
    ) -> Result<Option<DocumentInstance>, RepositoryError> {
        let schema = self
            .schema_registry
            .get(&document_type_id)
            .ok_or(RepositoryError::NotFound)?;

        let table = Table::from(&document_type_id);
        let mut columns: Vec<ColumnRef<'_>> = vec![
            Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            Cow::Borrowed(&CREATED_COLUMN),
            Cow::Borrowed(&UPDATED_COLUMN),
        ];

        if schema.has_draft_and_publish() {
            columns.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        }

        for (id, field) in schema.fields.iter() {
            let column = Column {
                qualifier: "m",
                name: id.normalized().as_ref(),
            };
            columns.push(Cow::Owned(column));
        }

        let query_builder =
            QueryBuilder::from(table)
                .select(columns)
                .where_condition(Condition::Equals {
                    column: Cow::Borrowed(&DOCUMENT_ID_COLUMN),
                    value: ConditionValue::Integer(id.0),
                });

        let (sql, params) = query_builder.build();
        println!("Generated SQL: {}", sql);

        let mut query_object = sqlx::query(&sql);
        for param in params {
            query_object = param.bind_to_query(query_object);
        }

        let row = query_object
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        ok(row.and_then(|row| self.row_to_document(&row, &schema).ok()))
    }

    async fn create(
        &self,
        document_type_id: DocumentTypeId,
        content: DocumentContent,
        user_id: Option<crate::domain::document::content::UserId>,
    ) -> Result<crate::domain::document::DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn update(
        &self,
        id: crate::domain::document::DocumentInstanceId,
        content_updates: std::collections::HashMap<
            String,
            crate::domain::document::content::ContentValue,
        >,
        user_id: Option<crate::domain::document::content::UserId>,
    ) -> Result<crate::domain::document::DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn delete(
        &self,
        document_type_id: luminair_common::DocumentTypeId,
        id: crate::domain::document::DocumentInstanceId,
    ) -> Result<(), RepositoryError> {
        todo!()
    }

    async fn publish(
        &self,
        id: crate::domain::document::DocumentInstanceId,
        user_id: Option<crate::domain::document::content::UserId>,
    ) -> Result<crate::domain::document::DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn unpublish(
        &self,
        id: crate::domain::document::DocumentInstanceId,
    ) -> Result<crate::domain::document::DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn count(&self, document_type_id: DocumentTypeId) -> Result<i64, RepositoryError> {
        let sql = format!(
            "SELECT COUNT(*) as count FROM \"{}\"",
            document_type_id.normalized()
        );

        let (count,) = sqlx::query_as::<_, (i64,)>(&sql)
            .fetch_one(self.pool.as_ref())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(count)
    }
}

impl PostgresDocumentRepository {
    pub fn new(
        schema_registry: &'static dyn DocumentTypesRegistry,
        database: &'static Database,
    ) -> Self {
        Self {
            schema_registry,
            database,
        }
    }

    fn row_to_document(
        &self,
        row: &sqlx::postgres::PgRow,
        schema: &DocumentType,
    ) -> Result<crate::domain::document::DocumentInstance, RepositoryError> {
        use chrono::{DateTime, Utc};
        use sqlx::Row;

        // Extract system fields
        let id: i64 = row
            .try_get(DOCUMENT_ID_FIELD_NAME)
            .map_err(|e| RepositoryError::DatabaseError(format!("Failed to parse id: {}", e)))?;
        let document_id = DocumentInstanceId(id);

        let created_at: DateTime<Utc> = row.try_get(CREATED_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse created_at: {}", e))
        })?;

        let updated_at: DateTime<Utc> = row.try_get(UPDATED_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse updated_at: {}", e))
        })?;

        // Parse publication state if the schema supports draft_and_publish
        let publication_state = if schema.has_draft_and_publish() {
            let published_at: Option<DateTime<Utc>> =
                row.try_get(PUBLISHED_FIELD_NAME).map_err(|e| {
                    RepositoryError::DatabaseError(format!("Failed to parse published_at: {}", e))
                })?;

            match published_at {
                Some(pub_at) => PublicationState::Published {
                    revision: 1,
                    published_at: pub_at,
                },
                None => PublicationState::Draft { revision: 1 },
            }
        } else {
            PublicationState::Published {
                revision: 1,
                published_at: created_at,
            }
        };

        // Extract field values
        let mut fields = std::collections::HashMap::new();
        for (field_id, field) in schema.fields.iter() {
            let normalized_name = field_id.normalized();
            let column_name = normalized_name.as_ref();

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
                    let date_value: Option<chrono::NaiveDate> =
                        row.try_get(column_name).map_err(|e| {
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
                    let datetime_value: Option<DateTime<Utc>> =
                        row.try_get(column_name).map_err(|e| {
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

            fields.insert(field_id.normalized().to_string(), value);
        }

        let content = DocumentContent {
            fields,
            publication_state,
        };

        let audit = AuditTrail {
            created_at,
            created_by: None,
            updated_at,
            updated_by: None,
            published_at: match publication_state {
                crate::domain::document::content::PublicationState::Published {
                    published_at,
                    ..
                } => Some(published_at),
                _ => None,
            },
            published_by: None,
            version: 1,
        };

        Ok(DocumentInstance {
            id: document_id,
            document_type_id: schema.id.clone(),
            content,
            audit,
        })
    }
}
