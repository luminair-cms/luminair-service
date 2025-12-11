use crate::domain::documents::Document;
use crate::domain::{AttributeId, DocumentId};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// A uniquely identifiable document Attribute.
#[derive(Debug)]
pub struct Attribute {
    pub id: AttributeId,
    pub body: AttributeBody
}

#[derive(Debug)]
pub enum AttributeBody {
    Field {
        attribute_type: AttributeType,
        unique: bool,
        required: bool,
        localized: bool,
        constraints: Option<AttributeConstraints>
    },
    Relation {
        relation_type: RelationType,
        target: RwLock<RelationTarget>,
        ordering: bool,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AttributeType {
    Uid,  // unique identifier based on text
    Uuid, // unique identifier based on UUID
    Text,
    Integer,
    Decimal,
    Date,
    DateTime,
    Boolean,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeConstraints {
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub minimal_length: Option<usize>,
    #[serde(default)]
    pub maximal_length: Option<usize>,
}

/// Relation attributes of bidirectional association
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RelationAttribute {
    /// in owning side, specifies the attribute on the inverse side
    InversedBy(AttributeId),
    /// in the inverse side, specifies the attribute on the owning side
    MappedBy(AttributeId),
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

#[derive(Clone, Debug)]
pub enum RelationTarget {
    Id(DocumentId),
    Ref(&'static Document)
}

// implementations

impl PartialEq for Attribute {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Attribute {}


/*
impl Attribute {
    pub fn target_document(&self) -> &Document {
        if let AttributeBody::Relation {target, ..} = self.body {
            let target_document_lock = target.read().unwrap();
            match target_document_lock.deref() {
                RelationTarget::Ref(d) => d,
                _ => panic!("Relation target must be a reference to a document, got {:?}", target.read().unwrap())
            }
        }
    }
}
 */

impl RelationType {
    pub fn is_owning(&self) -> bool {
        matches!(self, RelationType::HasOne | RelationType::HasMany)
    }
    pub fn is_inverse(&self) -> bool {
        matches!(self, RelationType::BelongsToOne | RelationType::BelongsToMany)
    }
}
