use std::collections::HashMap;

use crate::domain::repository::DocumentInstanceRepository;
use anyhow::Result;
use luminair_common::{
    entities::DocumentKind, DocumentType, DocumentTypesRegistry,
};

pub mod document;
pub mod repository;
pub mod sql;

/// This trait used only for testing purposes.
pub trait HelloService: Send + Sync + 'static {
    fn hello(&self) -> impl Future<Output = Result<String, anyhow::Error>> + Send;
}

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
    type H: HelloService;
    type R: DocumentInstanceRepository;

    fn hello_service(&self) -> &Self::H;
    fn document_types_registry(&self) -> &'static dyn DocumentTypesRegistry;
    fn documents_instance_repository(&self) -> &Self::R;

    /// Access the prebuilt index mapping API ids → document types.
    fn document_type_index(&self) -> &super::domain::DocumentTypeIndex;
}

/// An index built at service startup that maps every legal API identifier
/// (both plural and singular) to the corresponding `DocumentType` metadata.
///
/// We construct this once from the `DocumentTypesRegistry` and cache the
/// results in a hash map so that handlers can perform lookups cheaply on every
/// request without iterating the registry.
#[derive(Debug, Clone)]
pub struct DocumentTypeIndex {
    map: HashMap<String, &'static luminair_common::DocumentType>,
}

impl DocumentTypeIndex {
    /// Build the index from a registry reference.
    pub fn new(registry: &dyn DocumentTypesRegistry) -> Self {
        let mut map = HashMap::new();
        for dt in registry.iterate() {
            match dt.kind {
                DocumentKind::SingleType => {
                    map.insert(dt.info.singular_name.as_ref().to_string(), dt);
                }
                DocumentKind::Collection => {
                    map.insert(dt.info.plural_name.as_ref().to_string(), dt);
                }
            }
        }
        DocumentTypeIndex { map }
    }

    /// Look up an API id and return the associated `DocumentType` if it exists.
    pub fn lookup(&self, api_id: &str) -> Option<&'static DocumentType> {
        self.map.get(api_id).copied()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use luminair_common::test_utils::{make_collection, make_single, SimpleRegistry};

    #[test]
    fn index_contains_both_forms() {
        let dt1 = make_collection("foo");
        let dt2 = make_single("bar");
        let registry = SimpleRegistry { types: vec![dt1, dt2] };
        let idx = DocumentTypeIndex::new(&registry);

        // plural form for collection
        assert_eq!(idx.lookup("foos"), Some(dt1));
        // singular only for singletons
        assert_eq!(idx.lookup("bar"), Some(dt2));
        // plural of singleton not inserted (not needed)
        assert_eq!(idx.lookup("bars"), None);
        // missing key
        assert!(idx.lookup("baz").is_none());
    }
}
