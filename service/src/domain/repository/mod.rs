use std::{collections::HashMap, fmt::Debug};

use luminair_common::DocumentTypeId;

use crate::domain::{document::{DocumentContent, DocumentInstance, DocumentInstanceId, content::{ContentValue, UserId}}, repository::query::DocumentInstanceQuery};

pub mod query;

pub trait DocumentInstanceRepository: Send + Sync + Debug + 'static {
    /// Find instances matching query
    async fn find(
        &self,
        query: DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError>;
    
    /// Find single instance by ID
    async fn find_by_id(
        &self,
        document_type_id: DocumentTypeId,
        id: DocumentInstanceId,
    ) -> Result<Option<DocumentInstance>, RepositoryError>;
    
    /// Create new instance
    async fn create(
        &self,
        document_type_id: DocumentTypeId,
        content: DocumentContent,
        user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError>;
    
    /// Update instance
    async fn update(
        &self,
        id: DocumentInstanceId,
        content_updates: HashMap<String, ContentValue>,
        user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError>;
    
    /// Delete instance
    async fn delete(
        &self,
        document_type_id: DocumentTypeId,
        id: DocumentInstanceId,
    ) -> Result<(), RepositoryError>;
    
    /// Publish a draft
    async fn publish(
        &self,
        id: DocumentInstanceId,
        user_id: Option<UserId>,
    ) -> Result<DocumentInstance, RepositoryError>;
    
    /// Unpublish back to draft
    async fn unpublish(
        &self,
        id: DocumentInstanceId,
    ) -> Result<DocumentInstance, RepositoryError>;

    /// Get total count of documents
    async fn count(&self, collection_id: &str) -> Result<i64, RepositoryError>;
}

#[derive(Debug)]
pub enum RepositoryError {
    NotFound,
    ValidationFailed(String),
    DatabaseError(String),
    UniqueViolation(String),
}