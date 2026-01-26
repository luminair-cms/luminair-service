use std::collections::HashMap;
use crate::domain::attributes::{AttributeBody, AttributeType, RelationType};
use crate::domain::documents::Document;

use crate::domain::{AttributeId, DocumentId, DocumentRef};

/// Represents a Persistence structure on Document in Database
#[derive(Debug)]
pub struct PersistedDocument {
    pub has_draft_and_publish: bool,
    pub details: TableDetails,
    pub fields: HashMap<AttributeId, PersistedField>,
    pub relations: HashMap<AttributeId, PersistedRelation>,
}

#[derive(Debug)]
pub struct TableDetails {
    pub main_table_name: String,
    pub relation_column_name: String,
}

#[derive(Debug)]
pub struct PersistedField {
    pub attribute_type: AttributeType,
    pub unique: bool,
    pub required: bool,
    pub localized: bool,
    pub table_column_name: String
}

#[derive(Debug)]
pub struct PersistedRelation {
    pub relation_type: RelationType,
    pub target: DocumentRef,
    pub ordering: bool,
    pub relation_table_name: String
}

impl From<&'static Document> for TableDetails {
    fn from(value: &'static Document) -> Self {
        let main_table_name = value.id.normalized();
        let relation_column_name = format!("{}_id", value.info.singular_name.normalized());
        Self { main_table_name, relation_column_name }
    }
}

impl PersistedDocument {
    pub fn new(document: &'static Document, index: &HashMap<DocumentId, usize>) -> Self {
        let details = TableDetails::from(document);
        
        let mut fields = HashMap::new();
        let mut relations = HashMap::new();

        for attribute in document.attributes.iter() {
            let id = attribute.id.normalized();
            match &attribute.body {
                AttributeBody::Field {
                    attribute_type,
                    unique,
                    required,
                    localized,
                    ..
                } => {
                    let field = PersistedField {
                        attribute_type: *attribute_type,
                        unique: *unique,
                        required: *required,
                        localized: *localized,
                        table_column_name: id
                    };
                    fields.insert(attribute.id.clone(), field);
                }
                AttributeBody::Relation {
                    relation_type,
                    target,
                    ordering,
                } => {
                    let relation_table_name = format!("{}_{}_relation", &details.main_table_name, id);
                    let target = match index.get(target) {
                        Some(idx) => DocumentRef::from(*idx),
                        None => panic!("Don't found document for relation {}", target.as_ref())
                    };
                    let relation = PersistedRelation {
                        relation_type: *relation_type,
                        target,
                        ordering: *ordering,
                        relation_table_name
                    };
                    relations.insert(attribute.id.clone(), relation);
                }
            };
        };

        Self {
            has_draft_and_publish: document.has_draft_and_publish(),
            details,
            fields,
            relations
        }
    }
}
