use crate::{domain::{
        document::{
            DatabaseRowId, DocumentContent, DocumentInstance, DocumentInstanceId, content::ContentValue, lifecycle::UserId
        },
        repository::{DocumentInstanceRepository, RepositoryError, query::DocumentInstanceQuery},
    }, infrastructure::persistence::{columns::DOCUMENT_ID_COLUMN, infer::{main_query_builder, related_query_builder}, query::{
        Column, ColumnRef, Condition, ConditionValue, QueryBuilder,
    }, result::row_to_document}};

use luminair_common::{AttributeId, CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType, DocumentTypeId, DocumentTypesRegistry, ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME, database::Database, persistence::QualifiedTable};
use sqlx::Row;

use std::{borrow::Cow, collections::HashMap};

#[derive(Clone)]
pub struct PostgresDocumentRepository {
    schema_registry: &'static dyn DocumentTypesRegistry,
    database: &'static Database,
}

impl DocumentInstanceRepository for PostgresDocumentRepository {
    async fn find(
        &self,
        document_type: &DocumentType,
        query: DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let query_builder = main_query_builder(document_type);
        let (sql, params) = query_builder.build();

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
        let (sql, params) =
            main_query_builder(document_type).where_condition(Condition::Equals {
                column: Cow::Borrowed(&DOCUMENT_ID_COLUMN),
                value: ConditionValue::Uuid(id.0),
            }).build();

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
                let document = row_to_document(&row, document_type)?;
                Some(document)
            }
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
    
    async fn fetch_relations_for_one(
        &self,
        main_document_type: &DocumentType,
        main_table_id: DatabaseRowId,
        relation_fields: &[luminair_common::AttributeId],
    ) -> Result<HashMap<AttributeId, Vec<DocumentInstance>>, RepositoryError> {
        let ids = vec![main_table_id];
        let result = self.fetch_relations_for_many(main_document_type, &ids, relation_fields)
            .await?;
        
        let result = result.into_iter()
            .map(|(k,v)| {
                let v = v.into_values().next().unwrap();
                (k,v)
            })
            .collect();
        
        Ok(result)
    }
    
    async fn fetch_relations_for_many(
        &self,
        main_document_type: &DocumentType,
        main_table_ids: &[DatabaseRowId],
        relation_fields: &[luminair_common::AttributeId],
    ) -> Result<HashMap<AttributeId, HashMap<DocumentInstanceId, Vec<DocumentInstance>>>, RepositoryError>
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
            
            let (sql, params) =
                related_query_builder(main_document_type, 
                    related_document_type, 
                    &attr_id, 
                    main_table_ids).build();
            
            let mut query_object = sqlx::query(&sql);
            // TODO: refactor for using ANY() syntax
            for p in params {
                query_object = p.bind_to_query(query_object);
            }
            
            let mut rows = query_object.fetch(self.database.database_pool());
            
            use futures::TryStreamExt;
            while let Some(row) = rows
                .try_next()
                .await
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            {
                let document = row_to_document(&row, related_document_type)?;
                let owning: i64 = row
                    .try_get(ID_FIELD_NAME)
                    .map_err(|e| RepositoryError::DatabaseError(format!("Failed to parse id: {}", e)))?;
                let id = DatabaseRowId(owning);
            }

            // Group related docs by their owning main document id
            // For MVP simplicity, assume 1-to-N relations
            let mut grouped: HashMap<DocumentInstanceId, Vec<DocumentInstance>> =
                HashMap::new();
            for doc in related_docs {
                for main_id in instance_ids.iter() {
                    grouped
                        .entry(*main_id)
                        .or_insert_with(Vec::new)
                        .push(doc.clone());
                }
            }

            result.insert(attr_id.clone(), grouped);
        }

        Ok(result)
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
    
    /// Fetch related documents for a batch of main document instance IDs.
    /// Uses PostgreSQL = ANY syntax for batch queries.
    /// Results are sorted by document_instance_id for efficient joining.
    async fn fetch_related_documents(
        &self,
        main_schema: &DocumentType,
        related_schema: &DocumentType,
        relation_attr: &luminair_common::AttributeId,
        main_table_ids: &[DatabaseRowId],
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let (sql, params) =
            related_query_builder(main_schema, 
                related_schema, 
                relation_attr, 
                main_table_ids).build();

        let mut documents = Vec::new();
        let mut query_obj = sqlx::query(&sql);
        for p in params {
            query_obj = p.bind_to_query(query_obj);
        }
        let mut rows = query_obj.fetch(self.database.database_pool());
        use futures::TryStreamExt;
        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        {
            let document = self.row_to_document(&row, &related_schema)?;
            documents.push(document);
        }

        Ok(documents)
    }
}
