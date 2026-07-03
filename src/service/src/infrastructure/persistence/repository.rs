use crate::{
    domain::{document::{DocumentInstance, DocumentInstanceId, lifecycle::PublicationState}, query::{DocumentInstanceQuery, DocumentStatus}, repository::{DocumentsRepository, RelationMap, RelationOps, RepositoryError}},
    infrastructure::persistence::builders::{find::{query_count_documents, query_find_document_by_criteria, query_find_document_by_id}, relations::{delete_relation_entry, insert_relation_entry, query_find_related_documents}, write::{delete_document, insert_document, update_document, build_snapshot_insert}}
};

use futures::TryStreamExt;
use luminair_common::database::Database;
use luminair_common::{AttributeId, DocumentType, DocumentTypesRegistry, STATUS_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME, OWNING_DOCUMENT_ID_FIELD_NAME};
use sea_query::{DynIden, Expr};
use sea_query_sqlx::SqlxValues;
use sqlx::{AssertSqlSafe, Row};
use std::collections::HashMap;
use uuid::Uuid;
use crate::infrastructure::persistence::mapping::reader::row_to_document;

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
}

fn sqlx_query_with<'q>(sql: String, values: SqlxValues) -> sqlx::query::Query<'q, sqlx::Postgres, SqlxValues> {
    sqlx::query_with(AssertSqlSafe(sql), values)
}

impl DocumentsRepository for PostgresDocumentsRepository {
    async fn find(
        &self,
        document_type: &DocumentType,
        query: &DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let (sql, values) = query_find_document_by_criteria(document_type, query);
        let query_object = sqlx_query_with(sql, values);

        let mut rows = query_object.fetch(self.database.database_pool());
        let mut documents = Vec::new();

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

    async fn count(
        &self,
        document_type: &DocumentType,
        query: &DocumentInstanceQuery,
    ) -> Result<u64, RepositoryError> {
        let (sql, values) = query_count_documents(document_type, query);
        let row = sqlx_query_with(sql, values)
            .fetch_one(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let count: i64 = row
            .try_get(0)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(count as u64)
    }

    async fn find_by_id(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
        query: &DocumentInstanceQuery,
    ) -> Result<Option<DocumentInstance>, RepositoryError> {
        let (sql, values) = query_find_document_by_id(document_type, id.0, query);
        let query_object = sqlx_query_with(sql, values);

        let mut rows = query_object.fetch(self.database.database_pool());
        let mut documents = Vec::new();

        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        {
            let document = row_to_document(&row, document_type)?;
            documents.push(document);
        }

        Ok(documents.into_iter().next())
    }

    async fn fetch_relations(
        &self,
        document_type: &DocumentType,
        fields: &[AttributeId],
        filters: &HashMap<AttributeId, crate::domain::query::FilterExpression>,
        status: DocumentStatus,
        ids: &[DocumentInstanceId],
    ) -> Result<RelationMap, RepositoryError> {
        let mut result = HashMap::new();

        let params: Vec<Uuid> = ids.iter().map(|id| id.0).collect();

        for attr_id in fields {
            let rel_metadata = document_type.relations.get(attr_id).ok_or_else(|| {
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

            let rel_filter = filters.get(attr_id).unwrap_or(&crate::domain::query::FilterExpression::None);

            let (sql, values) = query_find_related_documents(
                document_type,
                related_document_type,
                attr_id,
                rel_filter,
                status,
                params.clone(),
            );
            let query_object = sqlx_query_with(sql, values);

            // Group related docs by their owning main document id (UUID)
            let mut grouped: HashMap<DocumentInstanceId, Vec<DocumentInstance>> = HashMap::new();

            let mut rows = query_object.fetch(self.database.database_pool());

            while let Some(row) = rows
                .try_next()
                .await
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            {
                let document = row_to_document(&row, related_document_type)?;
                let owning_uuid: Uuid = row.try_get(OWNING_DOCUMENT_ID_FIELD_NAME).map_err(|e| {
                    RepositoryError::DatabaseError(format!("Failed to parse owning_document_id: {}", e))
                })?;

                let id = DocumentInstanceId(owning_uuid);
                grouped.entry(id).or_insert_with(Vec::new).push(document);
            }

            result.insert(attr_id.clone(), grouped);
        }

        Ok(result)
    }

    async fn insert(
        &self,
        document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> Result<(), RepositoryError> {
        let has_draft_publish = document_type.has_draft_and_publish();

        if has_draft_publish {
            // Use Case 2: draft-and-publish is ON, creating a draft document
            self.insert_main_table(document_type, instance).await?;
        } else {
            // Use Case 1: draft-and-publish is OFF, creating a published document
            self.insert_main_table(document_type, instance).await?;
        }

        Ok(())
    }

    async fn update(
        &self,
        document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> Result<(), RepositoryError> {
        let has_draft_publish = document_type.has_draft_and_publish();
        let is_publishing = matches!(instance.content.publication_state, PublicationState::Published { .. });

        if has_draft_publish && is_publishing {
            // Use Case 3: draft-and-publish is ON, publishing
            // 1. Update main table (status -> PUBLISHED, revision, published_at, version)
            self.update_main_table(document_type, instance).await?;
            // 2. Copy row to snapshot table
            self.store_snapshot_for_published_instance(document_type, instance).await?;
        } else if has_draft_publish && !is_publishing {
            // Use Case 2: draft-and-publish is ON, saving a draft
            // 1. Update main table (status -> MODIFIED, clear published_at)
            self.update_main_table(document_type, instance).await?;
        } else {
            // Use Case 1: draft-and-publish is OFF
            // 1. Update main table (status is always PUBLISHED)
            self.update_main_table(document_type, instance).await?;
        }

        Ok(())
    }

    async fn delete(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> Result<(), RepositoryError> {
        let (sql, values) = delete_document(document_type, id.0);
        sqlx_query_with(sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn apply_relation_ops(
        &self,
        document_type: &DocumentType,
        document_id: DocumentInstanceId,
        ops: &HashMap<AttributeId, RelationOps>,
    ) -> Result<(), RepositoryError> {
        if ops.is_empty() {
            return Ok(());
        }

        // 1. For each relation attribute apply connect / disconnect using UUIDs directly
        for (attr_id, rel_ops) in ops {
            let rel_meta = document_type.relations.get(attr_id).ok_or_else(|| {
                RepositoryError::ValidationFailed(format!("Relation not found: {}", attr_id))
            })?;

            let _related_type = self
                .schema_registry
                .get(&rel_meta.target)
                .ok_or(RepositoryError::DocumentTypeNotFound)?;

            if !rel_ops.connect.is_empty() {
                for target_id in &rel_ops.connect {
                    let (sql, values) = insert_relation_entry(
                        document_type,
                        attr_id,
                        document_id.0,
                        target_id.0,
                    );
                    sqlx_query_with(sql, values)
                        .execute(self.database.database_pool())
                        .await
                        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
                }
            }

            if !rel_ops.disconnect.is_empty() {
                for target_id in &rel_ops.disconnect {
                    let (sql, values) = delete_relation_entry(
                        document_type,
                        attr_id,
                        document_id.0,
                        target_id.0,
                    );
                    sqlx_query_with(sql, values)
                        .execute(self.database.database_pool())
                        .await
                        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
                }
            }
        }

        Ok(())
    }
}

impl PostgresDocumentsRepository {


    async fn insert_main_table(
        &self,
        document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> Result<(), RepositoryError> {
        let revision: i32 = match &instance.content.publication_state {
            PublicationState::Published { revision, .. }
            | PublicationState::Draft { revision } => *revision,
        };

        let published_at = match &instance.content.publication_state {
            PublicationState::Published { published_at, .. } => Expr::from(*published_at),
            _ => Expr::null(),
        };

        let published_by = match &instance.content.publication_state {
            PublicationState::Published { published_by, .. } => {
                if let Some(user_id) = published_by {
                    Expr::from(user_id.to_string())
                } else {
                    Expr::null()
                }
            }
            _ => Expr::null(),
        };

        let mut params: Vec<Expr> = vec![
            instance.document_id.0.into(),
            Expr::from(self.main_status_value(document_type, instance).to_string()),
            instance.audit.created_at.into(),
            instance.audit.updated_at.into(),
            instance.audit.version.into(),
            revision.into(),
            published_at,
            published_by,
        ];

        for field in document_type.fields.iter() {
            match instance.content.fields.get(&field.id) {
                Some(val) => params.push(val.into()),
                None => params.push(Expr::null()),
            }
        }

        let (sql, values) = insert_document(document_type, params);
        sqlx_query_with(sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_main_table(
        &self,
        document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> Result<(), RepositoryError> {
        let mut column_values: Vec<(DynIden, Expr)> = vec![
            (UPDATED_FIELD_NAME.into(), instance.audit.updated_at.into()),
            (VERSION_FIELD_NAME.into(), instance.audit.version.into()),
            (
                STATUS_FIELD_NAME.into(),
                Expr::from(self.main_status_value(document_type, instance).to_string()),
            ),
        ];

        // Include publication state fields dynamically
        match &instance.content.publication_state {
            PublicationState::Published { revision, published_at, published_by } => {
                column_values.push((REVISION_FIELD_NAME.into(), (*revision).into()));
                column_values.push((PUBLISHED_FIELD_NAME.into(), (*published_at).into()));
                let by_expr = match published_by {
                    Some(user_id) => Expr::from(user_id.to_string()),
                    None => Expr::null(),
                };
                column_values.push((PUBLISHED_BY_FIELD_NAME.into(), by_expr));
            }
            PublicationState::Draft { revision } => {
                column_values.push((REVISION_FIELD_NAME.into(), (*revision).into()));
                column_values.push((PUBLISHED_FIELD_NAME.into(), Expr::null()));
                column_values.push((PUBLISHED_BY_FIELD_NAME.into(), Expr::null()));
            }
        }

        for field in document_type.fields.iter() {
            let expr = match instance.content.fields.get(&field.id) {
                Some(val) => val.into(),
                None => Expr::null(),
            };
            column_values.push((field.id.normalized().into(), expr));
        }

        let (sql, values) = update_document(document_type, instance.document_id.0, column_values);
        let result = sqlx_query_with(sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::DocumentInstanceNotFound);
        }
        Ok(())
    }

    async fn store_snapshot_for_published_instance(
        &self,
        document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> Result<(), RepositoryError> {
        let (sql, values) = build_snapshot_insert(document_type, instance);
        sqlx_query_with(sql, values)
            .execute(self.database.database_pool())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    fn main_status_value(
        &self,
        _document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> String {
        match &instance.content.publication_state {
            PublicationState::Published { .. } => "PUBLISHED".to_string(),
            PublicationState::Draft { revision } => {
                if *revision == 0 {
                    "DRAFT".to_string()
                } else {
                    "MODIFIED".to_string()
                }
            }
        }
    }
}
