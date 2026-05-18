use std::collections::HashMap;
use chrono::{DateTime, Utc};
use sea_query::Expr;
use sqlx::Row;
use uuid::{Timestamp, Uuid};
use luminair_common::database::Database;
use luminair_common::{AttributeId, DocumentType, DocumentTypesRegistry, ID_FIELD_NAME};
use crate::domain::document::{DatabaseRowId, DocumentInstance, DocumentInstanceId};
use crate::domain::document::content::{ContentValue, DocumentContent};
use crate::domain::document::lifecycle::{PublicationState, UserId};
use crate::domain::repository::{DocumentsRepository, RepositoryError};
use crate::domain::repository::query::DocumentInstanceQuery;
use crate::infrastructure::persistence::builders::{delete_document, delete_relation_entry, insert_document, insert_relation_entry, query_find_document_by_criteria, query_find_document_by_id, query_find_related_documents};
use crate::infrastructure::persistence::CLOCK_SEQUENCE;
use crate::infrastructure::persistence::result::row_to_document;

#[derive(Clone)]
pub struct PostgresDocumentsRepository {
    schema_registry: &'static dyn DocumentTypesRegistry,
    database: &'static Database,
}

impl PostgresDocumentsRepository {
    pub fn new(
        schema_registry: &'static dyn DocumentTypesRegistry,
        database: &'static Database,
    ) -> Self {
        Self {
            schema_registry,
            database,
        }
    }

    pub fn uuid_timestamp_from_chrono(datetime: &DateTime<Utc>) -> Timestamp {
        let secs = datetime.timestamp();
        let nanos = datetime.timestamp_subsec_nanos();
        Timestamp::from_unix(CLOCK_SEQUENCE, secs as u64, nanos)
    }
}

impl DocumentsRepository for PostgresDocumentsRepository {
    
    async fn find(
        &self,
        document_type: &DocumentType,
        query: DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let (sql, values) = query_find_document_by_criteria(document_type, &query);
        let query_object = sqlx::query_with(&sql, values);

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
        query: DocumentInstanceQuery,
        id: DocumentInstanceId,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let (sql, values) = query_find_document_by_id(document_type, id.0, &query);
        let query_object = sqlx::query_with(&sql, values);

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

    async fn fetch_relations_for_one(
        &self,
        main_document_type: &DocumentType,
        main_table_id: DatabaseRowId,
        relation_fields: &[AttributeId],
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
        relation_fields: &[AttributeId],
    ) -> Result<HashMap<AttributeId, HashMap<DatabaseRowId, Vec<DocumentInstance>>>, RepositoryError>
    {
        let mut result = HashMap::new();

        let params: Vec<i64> = main_table_ids.iter().map(|id| id.0).collect();

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
                .ok_or(RepositoryError::DocumentInstanceNotFound)?;

            let (sql, values) = query_find_related_documents(
                main_document_type,
                related_document_type,
                &attr_id,
                params.clone(),
            );
            let query_object = sqlx::query_with(&sql, values);

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

    async fn create(
        &self,
        document_type: &DocumentType,
        content: DocumentContent,
        user_id: Option<UserId>,
    ) -> Result<DocumentInstanceId, RepositoryError> {
        let now = chrono::Utc::now();
        let ts = Self::uuid_timestamp_from_chrono(&now);

        let document_id = DocumentInstanceId(Uuid::new_v7(ts));

        let mut params: Vec<Expr> = vec![
            document_id.0.into(),
            now.into(),  // CREATED
            now.into(),  // UPDATED
            1i32.into(), // VERSION
        ];

        if document_type.has_draft_and_publish() {
            match &content.publication_state {
                PublicationState::Published {
                    revision,
                    published_at,
                    published_by,
                } => {
                    params.push((*published_at).into());
                    params.push((*revision).into());
                }
                PublicationState::Draft { revision } => {
                    params.push(Expr::null());
                    params.push((*revision).into());
                }
            }
        }

        for field in &document_type.fields {
            let val = content.fields.get(&field.id);
            if let Some(val) = val {
                params.push(val.into());
            } else {
                params.push(Expr::null());
            }
        }

        let (sql, values) = insert_document(document_type, params);

        sqlx::query_with(&sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(document_id)
    }

    async fn update(
        &self,
        id: DocumentInstanceId,
        content_updates: HashMap<String, ContentValue>,
        user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn delete(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> Result<(), RepositoryError> {
        let (sql, values) = delete_document(document_type, id.0);
        sqlx::query_with(&sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn publish(
        &self,
        id: DocumentInstanceId,
        user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn connect(
        &self,
        document_type: &DocumentType,
        relation_attr: &AttributeId,
        owning_id: DatabaseRowId,
        inverse_id: DatabaseRowId,
    ) -> Result<(), RepositoryError> {
        let relation_metadata = document_type.relations.get(relation_attr).ok_or_else(|| {
            RepositoryError::ValidationFailed(format!("Relation not found: {}", relation_attr))
        })?;

        if !relation_metadata.relation_type.is_owning() {
            return Err(RepositoryError::ValidationFailed(format!(
                "Relation is not owning: {}",
                relation_attr
            )));
        }

        self.schema_registry
            .get(&relation_metadata.target)
            .ok_or(RepositoryError::DocumentTypeNotFound)?;

        let (sql, values) = insert_relation_entry(document_type, relation_attr, owning_id, inverse_id);
        sqlx::query_with(&sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn disconnect(
        &self,
        document_type: &DocumentType,
        relation_attr: &AttributeId,
        owning_id: DatabaseRowId,
        inverse_id: DatabaseRowId,
    ) -> Result<(), RepositoryError> {
        let relation_metadata = document_type.relations.get(relation_attr).ok_or_else(|| {
            RepositoryError::ValidationFailed(format!("Relation not found: {}", relation_attr))
        })?;

        if !relation_metadata.relation_type.is_owning() {
            return Err(RepositoryError::ValidationFailed(format!(
                "Relation is not owning: {}",
                relation_attr
            )));
        }

        self.schema_registry
            .get(&relation_metadata.target)
            .ok_or(RepositoryError::DocumentTypeNotFound)?;

        let (sql, values) = delete_relation_entry(document_type, relation_attr, owning_id, inverse_id);

        sqlx::query_with(&sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}