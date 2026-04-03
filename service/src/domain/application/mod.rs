pub mod implementation;

use crate::domain::document::content::ContentValue;
use crate::domain::document::{DocumentInstance, DocumentInstanceId};
use crate::domain::repository::query::DocumentInstanceQuery;
use crate::domain::repository::RepositoryError;
use luminair_common::{AttributeId, DocumentType, DocumentTypesRegistry};
use std::collections::HashMap;

pub trait DocumentServices: Send + Sync + 'static {
    /// Find instances matching query
    fn find(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        query: DocumentInstanceQuery,
    ) -> impl Future<Output = Result<Vec<DocumentInstance>, RepositoryError>> + Send;

    /// Find a single instance by ID
    fn find_by_id(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<Option<DocumentInstance>, RepositoryError>> + Send;
    
    /// Create a new instance
    fn create(
        &self,
        document_type: &DocumentType,
        fields: HashMap<AttributeId, ContentValue>,
    ) -> impl Future<Output = Result<DocumentInstanceId, RepositoryError>> + Send;

    /// Delete instance by ID
    fn delete(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
