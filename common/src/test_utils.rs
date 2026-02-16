use std::collections::HashMap;

use crate::domain::{DocumentType, DocumentTypeId, DocumentTypesRegistry};
use crate::entities::{DocumentKind, DocumentTypeInfo, DocumentTitle};

/// Simple registry storing a few static document types.
///
/// Public so that other crates can reuse it for their own tests.
#[derive(Debug)]
pub struct SimpleRegistry {
    pub types: Vec<&'static DocumentType>,
}

impl DocumentTypesRegistry for SimpleRegistry {
    fn iterate(&self) -> Box<dyn Iterator<Item = &'static DocumentType> + '_> {
        Box::new(self.types.iter().copied())
    }

    fn get(&self, id: &DocumentTypeId) -> Option<&'static DocumentType> {
        self.types.iter().copied().find(|dt| &dt.id == id)
    }
}

/// Helper for building a leaked `DocumentType` value.
pub fn make_type(
    id: &str,
    kind: DocumentKind,
    singular: &str,
    plural: &str,
) -> &'static DocumentType {
    let dt = DocumentType {
        id: DocumentTypeId::try_new(id).unwrap(),
        kind,
        info: DocumentTypeInfo {
            title: DocumentTitle::try_new(id).unwrap(),
            singular_name: DocumentTypeId::try_new(singular).unwrap(),
            plural_name: DocumentTypeId::try_new(plural).unwrap(),
            description: None,
        },
        options: None,
        fields: HashMap::new(),
        relations: HashMap::new(),
    };
    Box::leak(Box::new(dt))
}
/// Convenience for creating a collection-type with usual pluralization rule (`id` + "s").
pub fn make_collection(id: &str) -> &'static DocumentType {
    make_type(id, DocumentKind::Collection, id, &format!("{}s", id))
}

/// Convenience for creating a singleton-type (plural name still provided but
/// normally unused by index logic).
pub fn make_single(id: &str) -> &'static DocumentType {
    make_type(id, DocumentKind::SingleType, id, &format!("{}s", id))
}
