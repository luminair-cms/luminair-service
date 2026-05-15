use std::{collections::HashMap, fmt::Debug, future::Future};

use luminair_common::DocumentType;

use crate::domain::{
    document::{
        DatabaseRowId, DocumentInstance, DocumentInstanceId,
        content::{ContentValue, DocumentContent},
        lifecycle::UserId,
    },
    repository::query::DocumentInstanceQuery,
};

pub mod query;

pub trait DocumentsRepository: Send + Sync + 'static {
    /// Find instances matching query
    fn find(
        &self,
        document_type: &DocumentType,
        query: DocumentInstanceQuery,
    ) -> impl Future<Output = Result<Vec<DocumentInstance>, RepositoryError>> + Send;

    /// Find single document instance(s) by document ID
    fn find_by_id(
        &self,
        document_type: &DocumentType,
        query: DocumentInstanceQuery,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<Vec<DocumentInstance>, RepositoryError>> + Send;

    /// Fetch relations for one document instance
    fn fetch_relations_for_one(
        &self,
        main_document_type: &DocumentType,
        main_table_id: DatabaseRowId,
        relation_fields: &[luminair_common::AttributeId],
    ) -> impl Future<
        Output = Result<
            HashMap<luminair_common::AttributeId, Vec<DocumentInstance>>,
            RepositoryError,
        >,
    > + Send;

    /// Fetch relations in batch for multiple document instances
    fn fetch_relations_for_many(
        &self,
        main_document_type: &DocumentType,
        main_table_ids: &[DatabaseRowId],
        relation_fields: &[luminair_common::AttributeId],
    ) -> impl Future<
        Output = Result<
            HashMap<luminair_common::AttributeId, HashMap<DatabaseRowId, Vec<DocumentInstance>>>,
            RepositoryError,
        >,
    > + Send;

    /// Create new instance
    fn create(
        &self,
        document_type: &DocumentType,
        content: DocumentContent,
        user_id: Option<UserId>,
    ) -> impl Future<Output = Result<DocumentInstanceId, RepositoryError>> + Send;

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
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Publish a draft
    fn publish(
        &self,
        id: DocumentInstanceId,
        user_id: Option<UserId>,
    ) -> impl Future<Output = Result<DocumentInstance, RepositoryError>> + Send;

    /// Connect two related document instances for an owning relation
    fn connect(
        &self,
        document_type: &DocumentType,
        relation_attr: &luminair_common::AttributeId,
        owning_id: DatabaseRowId,
        inverse_id: DatabaseRowId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Disconnect two related document instances for an owning relation
    fn disconnect(
        &self,
        document_type: &DocumentType,
        relation_attr: &luminair_common::AttributeId,
        owning_id: DatabaseRowId,
        inverse_id: DatabaseRowId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}

#[derive(Debug)]
pub enum RepositoryError {
    DocumentTypeNotFound,
    DocumentInstanceNotFound,
    ValidationFailed(String),
    UniqueViolation(String),
    DatabaseError(String),
}
