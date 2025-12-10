use serde::{Deserialize, Serialize};
use crate::domain::AttributeId;

// structs

/// A uniquely identifiable document Attribute.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribute {
    pub id: AttributeId,
    pub attribute_type: AttributeType,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub localized: bool,
    pub constraints: Option<AttributeConstraints>
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
    pub pattern: Option<String>,
    pub minimal_length: Option<usize>,
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

// implementations

impl PartialEq for Attribute {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Attribute {}
