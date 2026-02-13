use std::borrow::Cow;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::Row;
use luminair_common::{CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentTypeId, DocumentTypesRegistry, ID_FIELD_NAME, PUBLISHED_FIELD_NAME, UPDATED_FIELD_NAME, database::Database, persistence::QualifiedTable, DocumentType, CREATED_BY_FIELD_NAME, UPDATED_BY_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, VERSION_FIELD_NAME, REVISION_FIELD_NAME};
use sqlx::types::{uuid, Uuid};
use luminair_common::entities::{AttributeType, DocumentField};
use crate::{
    domain::{
        document::{
            DocumentContent, DocumentInstance, DocumentInstanceId,
            content::{ContentValue, DomainValue},
            lifecycle::{AuditTrail, PublicationState, UserId},
        },
        repository::{DocumentInstanceRepository, RepositoryError, query::DocumentInstanceQuery},
    },
    infrastructure::persistence::query::{
        Column, ColumnRef, Condition, ConditionValue, QueryBuilder,
    },
};
use crate::domain::document::DatabaseRowId;

#[derive(Clone)]
pub struct PostgresDocumentRepository {
    schema_registry: &'static dyn DocumentTypesRegistry,
    database: &'static Database,
}

/// Common columns

const ID_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(ID_FIELD_NAME),
};
const DOCUMENT_ID_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(DOCUMENT_ID_FIELD_NAME),
};

impl DocumentInstanceRepository for PostgresDocumentRepository {
    async fn find(
        &self,
        query: DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let document_type_id = &query.document_type_id;
        let schema = self
            .schema_registry
            .get(document_type_id)
            .ok_or(RepositoryError::NotFound)?;

        let query_builder = Self::query_builder_from_schema(schema);
        let (sql, params) = query_builder.build();
        println!("Generated SQL: {}", sql);

        let mut query_object = sqlx::query(&sql);
        for param in params {
            query_object = param.bind_to_query(query_object);
        }

        let mut rows = query_object.fetch(self.database.database_pool());

        let mut documents = Vec::new();
        use futures::TryStreamExt;

        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        {
            let document = self.row_to_document(&row, &schema)?;
            documents.push(document);
        }

        Ok(documents)
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

        let query_builder = Self::query_builder_from_schema(schema)
                .where_condition(Condition::Equals {
                    column: Cow::Borrowed(&DOCUMENT_ID_COLUMN),
                    value: ConditionValue::Uuid(id.0),
                });

        let (sql, params) = query_builder.build();
        println!("Generated SQL: {}", sql);

        let mut query_object = sqlx::query(&sql);
        for param in params {
            query_object = param.bind_to_query(query_object);
        }

        let row = query_object
            .fetch_optional(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let document = match row {
            Some(row) => {
                let document = self.row_to_document(&row, &schema)?;
                Some(document)
            },
            None => None,
        };

        Ok(document)
    }

    async fn create(
        &self,
        _document_type_id: DocumentTypeId,
        _content: DocumentContent,
        _user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn update(
        &self,
        _id: DocumentInstanceId,
        _content_updates: std::collections::HashMap<
            String,
            ContentValue,
        >,
        _user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn delete(
        &self,
        _document_type_id: DocumentTypeId,
        _id: DocumentInstanceId,
    ) -> Result<(), RepositoryError> {
        todo!()
    }

    async fn publish(
        &self,
        _id: DocumentInstanceId,
        _user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn unpublish(
        &self,
        _id: DocumentInstanceId,
    ) -> Result<DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn count(&self, document_type_id: DocumentTypeId) -> Result<i64, RepositoryError> {
        let sql = format!(
            "SELECT COUNT(*) as count FROM \"{}\"",
            document_type_id.normalized()
        );

        let row: (i64,) = sqlx::query_as(&sql)
            .fetch_one(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(row.0)
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

            let value = Self::parse_field_value(row, field, column_name)?;

            fields.insert(field_id.normalized().to_string(), value);
        }

        let created_at: DateTime<Utc> = row.try_get(CREATED_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse created_at: {}", e))
        })?;

        let publication_state = Self::parse_publication_state(row, schema, created_at)?;
        let audit = Self::parse_audit_trail(row, created_at)?;

        let content = DocumentContent {
            fields,
            publication_state,
        };

        Ok(DocumentInstance {
            id,
            document_id,
            document_type_id: schema.id.clone(),
            content,
            audit,
        })
    }

    fn parse_field_value(row: &PgRow, field: &DocumentField, column_name: &str) -> Result<ContentValue, RepositoryError> {
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
        Ok(value)
    }

    fn parse_audit_trail(row: &PgRow, created_at: DateTime<Utc>) -> Result<AuditTrail, RepositoryError> {
        let updated_at: DateTime<Utc> = row.try_get(UPDATED_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse updated_at: {}", e))
        })?;

        let created_by: Option<String> = row.try_get(CREATED_BY_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse created_by: {}", e))
        })?;

        let updated_by: Option<String> = row.try_get(UPDATED_BY_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse updated_by: {}", e))
        })?;

        let version: i32 = row.try_get(VERSION_FIELD_NAME).map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to parse version: {}", e))
        })?;

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
    fn parse_publication_state(row: &PgRow, schema: &DocumentType, created_at: DateTime<Utc>) -> Result<PublicationState, RepositoryError> {
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

    /// Common columns

    const CREATED_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(CREATED_FIELD_NAME),
    };
    const UPDATED_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(UPDATED_FIELD_NAME),
    };
    const PUBLISHED_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(PUBLISHED_FIELD_NAME),
    };

    const CREATED_BY_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(CREATED_BY_FIELD_NAME),
    };
    const UPDATED_BY_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(UPDATED_BY_FIELD_NAME),
    };
    const PUBLISHED_BY_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(PUBLISHED_BY_FIELD_NAME),
    };

    const VERSION_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(VERSION_FIELD_NAME),
    };
    const REVISION_COLUMN: Column<'static> = Column {
        qualifier: "m",
        name: Cow::Borrowed(REVISION_FIELD_NAME),
    };

    fn query_builder_from_schema(schema: &DocumentType) -> QueryBuilder<'_> {
        let table = QualifiedTable::from(schema);
        let mut columns: Vec<ColumnRef<'_>> = vec![
            Cow::Borrowed(&ID_COLUMN),
            Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            Cow::Borrowed(&Self::CREATED_COLUMN),
            Cow::Borrowed(&Self::UPDATED_COLUMN),
            Cow::Borrowed(&Self::CREATED_BY_COLUMN),
            Cow::Borrowed(&Self::UPDATED_BY_COLUMN),
            Cow::Borrowed(&Self::VERSION_COLUMN),
        ];

        if schema.has_draft_and_publish() {
            columns.push(Cow::Borrowed(&Self::PUBLISHED_COLUMN));
            columns.push(Cow::Borrowed(&Self::PUBLISHED_BY_COLUMN));
            columns.push(Cow::Borrowed(&Self::REVISION_COLUMN));
        }

        for id in schema.fields.keys() {
            let column = Column {
                qualifier: "m",
                name: Cow::Owned(id.normalized()),
            };
            columns.push(Cow::Owned(column));
        }

        QueryBuilder::from(table).select(columns)
    }
}
