use luminair_common::{DocumentTypesRegistry, database::Database};

use crate::domain::repository::{DocumentInstanceRepository, RepositoryError};

#[derive(Clone)]
pub struct PostgresDocumentRepository {
    pub schema_registry: &'static dyn DocumentTypesRegistry,
    database: &'static Database,
}

impl DocumentInstanceRepository for PostgresDocumentRepository {
    async fn find(
        &self,
        query: crate::domain::repository::query::DocumentInstanceQuery,
    ) -> Result<Vec<crate::domain::document::DocumentInstance>, RepositoryError> {
        let schema = self
            .schema_registry
            .get(&query.document_type_id)
            .ok_or(RepositoryError::NotFound)?;
        
        todo!()
    }

    async fn find_by_id(
        &self,
        document_type_id: luminair_common::DocumentTypeId,
        id: crate::domain::document::DocumentInstanceId,
    ) -> Result<Option<crate::domain::document::DocumentInstance>, RepositoryError> {
        let schema = self
            .schema_registry
            .get(&document_type_id)
            .ok_or(RepositoryError::NotFound)?;
        
        todo!()
    }

    async fn create(
        &self,
        document_type_id: luminair_common::DocumentTypeId,
        content: crate::domain::document::DocumentContent,
        user_id: Option<crate::domain::document::content::UserId>,
    ) -> Result<crate::domain::document::DocumentInstance, RepositoryError> {
        todo!()
    }

    async fn update(
        &self,
        id: crate::domain::document::DocumentInstanceId,
        content_updates: std::collections::HashMap<String, crate::domain::document::content::ContentValue>,
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

    async fn count(&self, collection_id: &str) -> Result<i64, RepositoryError> {
        todo!()
    }
}