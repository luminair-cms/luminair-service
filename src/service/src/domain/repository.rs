use std::{collections::HashMap, future::Future};

use luminair_common::{AttributeId, DocumentType};

use crate::domain::{
    document::{DatabaseRowId, DocumentInstance, DocumentInstanceId},
    query::{DocumentInstanceQuery, DocumentStatus},
};

/// Port: the persistence contract that infrastructure adapters must implement.
///
/// All business logic (publication state machine, relation validation) lives in
/// the application service layer. The repository is pure persistence — it saves
/// and loads domain objects without interpreting their meaning.
///
/// ## Key design principles
///
/// - `insert` and `update` accept a fully-constructed [`DocumentInstance`].
///   The service is responsible for building valid instances; the repository only
///   persists them.
/// - `find_by_id` returns `Option<DocumentInstance>` because the combination of
///   UUID + status filter identifies at most one row.
/// - `fetch_relations` is the single batch relation-loading method, replacing the
///   previous `fetch_relations_for_one` / `fetch_relations_for_many` pair.
/// - `apply_relation_ops` accepts [`DocumentInstanceId`] (UUID) values; the
///   repository resolves them to internal row IDs internally via batch SELECTs.
pub trait DocumentsRepository: Send + Sync + 'static {
    // ── Read ────────────────────────────────────────────────────────────────

    /// Return all instances matching the query.
    fn find(
        &self,
        document_type: &DocumentType,
        query: &DocumentInstanceQuery,
    ) -> impl Future<Output = Result<Vec<DocumentInstance>, RepositoryError>> + Send;

    /// Return the total number of instances matching the query.
    /// Used for accurate pagination metadata.
    fn count(
        &self,
        document_type: &DocumentType,
        query: &DocumentInstanceQuery,
    ) -> impl Future<Output = Result<u64, RepositoryError>> + Send;

    /// Return the single instance identified by `id`, or `None` if not found.
    ///
    /// The `query` parameter carries the publication status filter.
    /// For draft-and-publish types, `status=published` returns the published row
    /// and `status=draft` returns the latest draft row.
    fn find_by_id(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
        query: &DocumentInstanceQuery,
    ) -> impl Future<Output = Result<Option<DocumentInstance>, RepositoryError>> + Send;

    /// Batch-load relations for a set of main document rows.
    ///
    /// Returns a nested map: `attribute_id → owning_document_id → related_instances`.
    fn fetch_relations(
        &self,
        document_type: &DocumentType,
        fields: &[AttributeId],
        status: DocumentStatus,
        ids: &[DocumentInstanceId],
    ) -> impl Future<Output = Result<RelationMap, RepositoryError>> + Send;

    // ── Write ───────────────────────────────────────────────────────────────

    /// Persist a newly created document instance.
    ///
    /// The `instance.id` (database row key) is a placeholder; the database
    /// assigns the actual row ID. All other fields — `document_id`, `audit`,
    /// `content`, `publication_state` — are taken from the instance as-is.
    fn insert(
        &self,
        document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Persist changes to an existing document instance.
    ///
    /// Identifies the row to update via `instance.document_id`.
    fn update(
        &self,
        document_type: &DocumentType,
        instance: &DocumentInstance,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Delete the instance identified by `id`.
    fn delete(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Apply connect / disconnect relation operations atomically.
    ///
    /// Resolves every [`DocumentInstanceId`] to its internal database row ID
    /// via batch SELECT queries — callers never need to manage row IDs.
    fn apply_relation_ops(
        &self,
        document_type: &DocumentType,
        document_id: DocumentInstanceId,
        ops: &HashMap<AttributeId, RelationOps>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}

// ── Supporting types ─────────────────────────────────────────────────────────

/// `attribute_id → owning_document_id → related_instances`
pub type RelationMap = HashMap<AttributeId, HashMap<DocumentInstanceId, Vec<DocumentInstance>>>;

/// Connect / disconnect sets for a single relation attribute.
#[derive(Debug, Default)]
pub struct RelationOps {
    /// UUIDs of documents to add to the relation.
    pub connect: Vec<DocumentInstanceId>,
    /// UUIDs of documents to remove from the relation.
    pub disconnect: Vec<DocumentInstanceId>,
}

/// Errors that can be returned by any repository method.
#[derive(thiserror::Error, Debug)]
pub enum RepositoryError {
    #[error("Document type not found")]
    DocumentTypeNotFound,
    #[error("Document instance not found")]
    DocumentInstanceNotFound,
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("Unique constraint violated: {0}")]
    UniqueViolation(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
}
