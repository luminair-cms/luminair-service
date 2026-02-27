use std::collections::HashMap;

use crate::AttributeId;
use crate::domain::{DocumentType, DocumentTypeId, DocumentTypesRegistry};
use crate::entities::{FieldType, DocumentField, DocumentKind, DocumentTitle, DocumentTypeInfo};

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
pub fn make_document_type(
    id: &str,
    kind: DocumentKind,
    singular: &str,
    plural: &str,
    fields: Vec<(&str, DocumentField)>,
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
        fields: fields
            .into_iter()
            .map(|(name, field)| (AttributeId::try_new(name.to_owned()).unwrap(), field))
            .collect(),
        relations: HashMap::new(),
    };
    Box::leak(Box::new(dt))
}

/// Convenience for creating a collection-type with usual pluralization rule (`id` + "s").
pub fn make_collection(id: &str) -> &'static DocumentType {
    make_document_type(id, DocumentKind::Collection, id, &format!("{}s", id), Vec::new())
}

/// Convenience for creating a singleton-type with usual pluralization rule (`id` + "s").
pub fn make_single(id: &str) -> &'static DocumentType {
    make_document_type(id, DocumentKind::SingleType, id, &format!("{}s", id), Vec::new())
}

pub fn make_uid_document_field() -> (&'static str, DocumentField) {
    ("uid", DocumentField { field_type: FieldType::Uid, unique: true, required: true, constraints: None })
}

pub fn make_document_fields() -> Vec<(&'static str, DocumentField)> {
    vec![
        make_uid_document_field(),
        ("name", DocumentField { field_type: FieldType::Text { localized: false}, unique: true, required: true, constraints: None }),
        ("description", DocumentField { field_type: FieldType::Text { localized: true}, unique: false, required: false,  constraints: None }),
        ("amount", DocumentField { field_type: FieldType::Decimal, unique: false, required: true, constraints: None }),
        ("metadata", DocumentField { field_type: FieldType::Json, unique: false, required: false, constraints: None }),
        ]
}