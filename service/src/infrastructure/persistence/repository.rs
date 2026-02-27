use crate::{
    domain::{
        document::{
            DatabaseRowId, DocumentContent, DocumentInstance, DocumentInstanceId, content::{self, ContentValue}, lifecycle::UserId
        },
        repository::{DocumentInstanceRepository, RepositoryError, query::DocumentInstanceQuery},
    },
    infrastructure::persistence::{
        build::{main_query_builder, related_query_builder},
        columns::DOCUMENT_ID_COLUMN,
        result::row_to_document,
    },
};

use luminair_common::{
    database::Database, AttributeId, DocumentType, DocumentTypeId, DocumentTypesRegistry,
    ID_FIELD_NAME,
};
use sqlx::Row;

use crate::infrastructure::persistence::columns::OWNING_ID_COLUMN;
use std::{borrow::Cow, collections::HashMap};
use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use uuid::{ContextV7, Timestamp};
use crate::domain::document::lifecycle::PublicationState;
use crate::domain::sql::query::Condition;
use crate::infrastructure::persistence::build::build_create_statement;
use crate::infrastructure::persistence::parameters::SqlParametersHolder;

#[derive(Clone)]
pub struct PostgresDocumentRepository {
    schema_registry: &'static dyn DocumentTypesRegistry,
    database: &'static Database,
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
}

impl DocumentInstanceRepository for PostgresDocumentRepository {
    async fn find(
        &self,
        document_type: &DocumentType,
        _query: DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let query_builder = main_query_builder(document_type);
        let (sql, _) = query_builder.build();

        let query_object = sqlx::query(&sql);
        let mut rows = query_object.fetch(self.database.database_pool());

        let mut documents = Vec::new();
        use futures::TryStreamExt;

        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        {
            let document = row_to_document(&row, document_type)?;
            documents.push(document);
        }

        Ok(documents)
    }

    async fn find_by_id(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> Result<Option<DocumentInstance>, RepositoryError> {
        let mut params_holder = SqlParametersHolder::new();

        let (sql, params) = main_query_builder(document_type)
            .where_condition(Condition::Equals {
                column: Cow::Borrowed(&DOCUMENT_ID_COLUMN),
                value: params_holder.bind(id.0),
            })
            .build();

        let query_arguments = params_holder.generate_args(&params);
        let query_object = sqlx::query_with(&sql, query_arguments);

        let row = query_object
            .fetch_optional(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let document = match row {
            Some(row) => {
                let document = row_to_document(&row, document_type)?;
                Some(document)
            }
            None => None,
        };

        Ok(document)
    }

    async fn create(
        &self,
        document_type: &DocumentType,
        content: DocumentContent,
        user_id: Option<UserId>,
    ) -> Result<DocumentInstanceId, RepositoryError> {
        let mut params_holder = SqlParametersHolder::new();
        let mut create_statement = build_create_statement(document_type);

        let mut param_refs = Vec::new();

        let now = chrono::Utc::now();
        let ts = uuid_timestamp_from_chrono(&now);

        let document_id = DocumentInstanceId(Uuid::new_v7(ts));

        param_refs.push(params_holder.bind(document_id.0)); // DOCUMENT_ID_FIELD_NAME
        param_refs.push(params_holder.bind(now)); // CREATED_FIELD_NAME
        param_refs.push(params_holder.bind(now)); // UPDATED_FIELD_NAME

        let uid_str = user_id.as_ref().map(|u| u.0.clone());
        param_refs.push(params_holder.bind_null()); // CREATED_BY_FIELD_NAME
        param_refs.push(params_holder.bind_null()); // UPDATED_BY_FIELD_NAME

        param_refs.push(params_holder.bind(1i32)); // VERSION_FIELD_NAME

        if document_type.has_draft_and_publish() {
            match &content.publication_state {
                PublicationState::Published { revision, published_at, published_by } => {
                    param_refs.push(params_holder.bind(*published_at));
                    param_refs.push(params_holder.bind_null());
                    param_refs.push(params_holder.bind(*revision));
                }
                PublicationState::Draft { revision } => {
                    param_refs.push(params_holder.bind_null());
                    param_refs.push(params_holder.bind_null());
                    param_refs.push(params_holder.bind(*revision));
                }
            }
        }

        for (id, _field_def) in &document_type.fields {
            let val = content.fields.get(id.as_ref());
            match val {
                Some(content) => {
                    param_refs.push(params_holder.bind(content.clone()));
                }
                _ => {
                    param_refs.push(params_holder.bind_null());
                }
            }
        }

        let (sql, _) = create_statement.with_params(param_refs.clone()).to_sql();
        let query_arguments = params_holder.generate_args(&param_refs);

        sqlx::query_with(&sql, query_arguments)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(document_id)
    }

    async fn update(
        &self,
        _id: DocumentInstanceId,
        _content_updates: std::collections::HashMap<String, ContentValue>,
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

    async fn fetch_relations_for_one(
        &self,
        main_document_type: &DocumentType,
        main_table_id: DatabaseRowId,
        relation_fields: &[luminair_common::AttributeId],
    ) -> Result<HashMap<AttributeId, Vec<DocumentInstance>>, RepositoryError> {
        let ids = vec![main_table_id];
        let result = self
            .fetch_relations_for_many(main_document_type, &ids, relation_fields)
            .await?;

        let result = result
            .into_iter()
            .map(|(k, v)| {
                let v = v.into_values().next().unwrap();
                (k, v)
            })
            .collect();

        Ok(result)
    }

    async fn fetch_relations_for_many(
        &self,
        main_document_type: &DocumentType,
        main_table_ids: &[DatabaseRowId],
        relation_fields: &[luminair_common::AttributeId],
    ) -> Result<HashMap<AttributeId, HashMap<DatabaseRowId, Vec<DocumentInstance>>>, RepositoryError>
    {
        let mut result = HashMap::new();

        for attr_id in relation_fields {
            let rel_metadata = main_document_type.relations.get(attr_id).ok_or_else(|| {
                RepositoryError::ValidationFailed(format!("Relation not found: {}", attr_id))
            })?;

            if !rel_metadata.relation_type.is_owning() {
                return Err(RepositoryError::ValidationFailed(format!(
                    "Relation is not owning: {}",
                    attr_id
                )));
            }

            let related_document_type = self
                .schema_registry
                .get(&rel_metadata.target)
                .ok_or(RepositoryError::NotFound)?;

            let mut params_holder = SqlParametersHolder::new();

            let values: Vec<i64> = main_table_ids.iter().map(|id| id.0).collect();

            let (sql, params) =
                related_query_builder(main_document_type, related_document_type, &attr_id)
                    .where_condition(Condition::EqualsAny {
                        column: Cow::Borrowed(&OWNING_ID_COLUMN),
                        value: params_holder.bind(values),
                    })
                    .build();

            let query_arguments = params_holder.generate_args(&params);
            let query_object = sqlx::query_with(&sql, query_arguments);

            // Group related docs by their owning main document id
            // For MVP simplicity, assume 1-to-N relations
            let mut grouped: HashMap<DatabaseRowId, Vec<DocumentInstance>> = HashMap::new();

            let mut rows = query_object.fetch(self.database.database_pool());

            use futures::TryStreamExt;
            while let Some(row) = rows
                .try_next()
                .await
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            {
                let document = row_to_document(&row, related_document_type)?;
                let owning_id: i64 = row.try_get(ID_FIELD_NAME).map_err(|e| {
                    RepositoryError::DatabaseError(format!("Failed to parse id: {}", e))
                })?;

                let id = DatabaseRowId(owning_id);
                grouped.entry(id).or_insert_with(Vec::new).push(document);
            }

            result.insert(attr_id.clone(), grouped);
        }

        Ok(result)
    }
}

const CLOCK_SEQUENCE: ContextV7 = ContextV7::new();

fn uuid_timestamp_from_chrono(datetime: &DateTime<Utc>) -> Timestamp {
    let secs = datetime.timestamp();
    let nanos = datetime.timestamp_subsec_nanos();
    Timestamp::from_unix(CLOCK_SEQUENCE, secs as u64, nanos)
}
