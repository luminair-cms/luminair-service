//! Test support types for the domain layer.
//!
//! Available only when the `test-helpers` feature is enabled.
//! Never compiled into production builds.

use std::collections::HashMap;

use crate::domain::{DocumentTypeApiId, DocumentTypeId, DocumentTypesRegistry};
use crate::entities::{DocumentType, DocumentKind};

/// A lightweight in-memory [`DocumentTypesRegistry`] for use in integration tests.
///
/// - Owns its [`DocumentType`] values — no `Box::leak` for child objects.
/// - O(1) [`get`] and [`lookup`] via `HashMap`.
/// - Thread-safe: `DocumentType: Send + Sync`, so this registry is too.
///
/// # Example
/// ```rust
/// let registry = InMemoryDocumentTypesRegistry::from_vec(vec![
///     DocumentType::new_bare_collection("article", "article", "articles").unwrap(),
/// ]);
/// ```
#[derive(Debug)]
pub struct InMemoryDocumentTypesRegistry {
    by_id:     HashMap<DocumentTypeId, DocumentType>,
    by_api_id: HashMap<String, DocumentTypeId>,
}

impl InMemoryDocumentTypesRegistry {
    /// Builds a registry from an owned list of document types.
    pub fn from_vec(docs: Vec<DocumentType>) -> Self {
        let mut by_id     = HashMap::with_capacity(docs.len());
        let mut by_api_id = HashMap::with_capacity(docs.len());

        for doc in docs {
            let api_key = match doc.kind {
                DocumentKind::SingleType => doc.info.singular_name.as_ref().to_string(),
                DocumentKind::Collection => doc.info.plural_name.as_ref().to_string(),
            };
            by_api_id.insert(api_key, doc.id.clone());
            by_id.insert(doc.id.clone(), doc);
        }

        Self { by_id, by_api_id }
    }
}

impl DocumentTypesRegistry for InMemoryDocumentTypesRegistry {
    fn iterate(&self) -> Box<dyn Iterator<Item = &DocumentType> + '_> {
        Box::new(self.by_id.values())
    }

    fn get(&self, id: &DocumentTypeId) -> Option<&DocumentType> {
        self.by_id.get(id)
    }

    fn lookup(&self, api_id: &DocumentTypeApiId) -> Option<&DocumentType> {
        self.by_api_id
            .get(api_id.as_ref())
            .and_then(|id| self.by_id.get(id))
    }
}

