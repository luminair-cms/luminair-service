use crate::domain::{AttributeId, DocumentId};
use serde::{Deserialize, Serialize};

/// A uniquely identifiable document Field.
#[derive(Debug)]
pub struct DocumentField {
    pub attribute_type: AttributeType,
    pub unique: bool,
    pub required: bool,
    pub localized: bool,
    pub constraints: Option<AttributeConstraints>,
    pub table_column_name: String,
}

/// A uniquely identifiable document Relation.
#[derive(Debug)]
pub struct DocumentRelation {
    pub relation_type: RelationType,
    pub target: DocumentId,
    pub ordering: bool,
    pub relation_table_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RelationType {
    // owning side
    HasOne,
    HasMany,
    // inverse side
    BelongsToOne,
    BelongsToMany,
}

// implementations

impl RelationType {
    pub fn is_owning(&self) -> bool {
        matches!(self, RelationType::HasOne | RelationType::HasMany)
    }
    pub fn is_inverse(&self) -> bool {
        matches!(
            self,
            RelationType::BelongsToOne | RelationType::BelongsToMany
        )
    }
}
