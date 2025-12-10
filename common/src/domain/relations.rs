use crate::domain::documents::Document;
use crate::domain::{AttributeId, DocumentId};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::RwLock;
// structs

/// A uniquely identifiable document Attribute.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub id: RelationId,
    pub relation_type: RelationType,
    pub target: RwLock<RelationTarget>,
    pub ordering: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RelationType {
    // owning side
    HasOne,
    HasMany,
    // inverse side
    BelongsToOne,
    BelongsToMany,
}

pub type RelationId = AttributeId;

#[derive(Clone, Debug, Serialize)]
pub enum RelationTarget {
    Id(DocumentId),
    Ref(&'static Document)
}

// implementations

impl PartialEq for Relation {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Relation {}

impl Relation {
    pub fn target_document(&self) -> &Document {
        let target_document_lock = self.target.read().unwrap();
        match target_document_lock.deref() {
            RelationTarget::Ref(d) => d,
            _ => panic!("Relation target must be a reference to a document, got {:?}", self.target.read().unwrap())
        }
    }
}

impl RelationType {
    pub fn is_owning(&self) -> bool {
        matches!(self, RelationType::HasOne | RelationType::HasMany)
    }
    pub fn is_inverse(&self) -> bool {
        matches!(self, RelationType::BelongsToOne | RelationType::BelongsToMany)
    }
}
