use std::{collections::HashMap, fmt::Debug, future::Future};

use luminair_common::{DocumentType, DocumentTypeId};


use crate::domain::{
    document::{
        DatabaseRowId, DocumentContent, DocumentInstance, DocumentInstanceId, content::ContentValue, lifecycle::UserId
    },
    repository::query::DocumentInstanceQuery,
};

pub mod query;

pub trait DocumentInstanceRepository: Send + Sync + 'static {
    /// Find instances matching query
    fn find(
        &self,
        document_type: &DocumentType,
        query: DocumentInstanceQuery,
    ) -> impl Future<Output = Result<Vec<DocumentInstance>, RepositoryError>> + Send;

    /// Find single instance by ID
    fn find_by_id(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<Option<DocumentInstance>, RepositoryError>> + Send;

    /// Create new instance
    fn create(
        &self,
        document_type_id: DocumentTypeId,
        content: DocumentContent,
        user_id: Option<UserId>,
    ) -> impl Future<Output = Result<DocumentInstance, RepositoryError>> + Send;

    /// Update instance
    fn update(
        &self,
        id: DocumentInstanceId,
        content_updates: HashMap<String, ContentValue>,
        user_id: Option<UserId>,
    ) -> impl Future<Output = Result<DocumentInstance, RepositoryError>> + Send;

    /// Delete instance
    fn delete(
        &self,
        document_type_id: DocumentTypeId,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Publish a draft
    fn publish(
        &self,
        id: DocumentInstanceId,
        user_id: Option<UserId>,
    ) -> impl Future<Output = Result<DocumentInstance, RepositoryError>> + Send;

    /// Unpublish back to draft
    fn unpublish(
        &self,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<DocumentInstance, RepositoryError>> + Send;

    /// Get total count of documents
    fn count(
        &self,
        document_type_id: DocumentTypeId,
    ) -> impl Future<Output = Result<i64, RepositoryError>> + Send;

    /// Fetch relations for one document instance
    fn fetch_relations_for_one(
        &self,
        main_document_type: &DocumentType,
        main_table_id: DatabaseRowId,
        relation_fields: &[luminair_common::AttributeId],
    ) -> impl Future<Output = Result<HashMap<luminair_common::AttributeId, Vec<DocumentInstance>>, RepositoryError>> + Send;

    /// Fetch relations in batch for multiple document instances
    fn fetch_relations_for_many(
        &self,
        main_document_type: &DocumentType,
        main_table_ids: &[DatabaseRowId],
        relation_fields: &[luminair_common::AttributeId],
    ) -> impl Future<Output = Result<HashMap<luminair_common::AttributeId, HashMap<DatabaseRowId, Vec<DocumentInstance>>>, RepositoryError>> + Send;
}

#[derive(Debug)]
pub enum RepositoryError {
    NotFound,
    ValidationFailed(String),
    UniqueViolation(String),
    DatabaseError(String),
}
