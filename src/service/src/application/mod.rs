pub mod implementation;
pub mod error;
pub mod commands;
pub mod service;

use std::collections::HashMap;
use std::future::Future;

use luminair_common::{AttributeId, DocumentType, DocumentTypesRegistry};

use crate::domain::document::{DocumentInstance, DocumentInstanceId, content::ContentValue};
use crate::domain::query::DocumentInstanceQuery;
use crate::domain::repository::{RelationOps, RepositoryError};

/// The global application state shared between all HTTP request handlers.
///
/// `AppState` is a composition-root concern: it wires the HTTP adapters to the
/// application service layer. It lives here rather than in the domain root because
/// it references [`DocumentsService`], which is an application-layer contract.
pub trait AppState: Clone + Send + Sync + 'static {
    type D: DocumentsService;

    fn document_types(&self) -> &'static dyn DocumentTypesRegistry;

    fn documents_service(&self) -> &Self::D;
}

/// Application-layer service contract for document operations.
///
/// This trait is the primary entry point for all document lifecycle operations.
/// It sits between the HTTP adapters (which call it) and the repository port
/// (which it calls). Business rules — validation, publication state, relation
/// ownership checks — live here, not in the handlers or the repository.
pub trait DocumentsService: Send + Sync + 'static {
    /// Return all instances matching the query, together with the total count
    /// for pagination metadata. The total reflects all matching rows, not just
    /// the current page.
    fn find(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        query: DocumentInstanceQuery,
    ) -> impl Future<Output = Result<Vec<DocumentInstance>, RepositoryError>> + Send;

    /// Return the single instance identified by `id`, or `None` if not found.
    fn find_by_id(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        query: DocumentInstanceQuery,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<Option<DocumentInstance>, RepositoryError>> + Send;

    /// Create a new document instance from the supplied field values.
    /// Returns the stable UUID assigned to the new instance.
    fn create(
        &self,
        document_type: &DocumentType,
        fields: HashMap<AttributeId, ContentValue>,
    ) -> impl Future<Output = Result<DocumentInstanceId, RepositoryError>> + Send;

    /// Delete the instance identified by `id`.
    fn delete(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Apply connect / disconnect relation operations for a document instance.
    ///
    /// The `document_id` is the stable UUID of the owning document. All related
    /// document IDs in `ops` are also UUIDs — no internal row IDs are exposed.
    ///
    /// The service validates that every attribute in `ops` is a declared owning
    /// relation before delegating to the repository.
    fn modify_relations(
        &self,
        document_type: &DocumentType,
        document_id: DocumentInstanceId,
        ops: HashMap<AttributeId, RelationOps>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
