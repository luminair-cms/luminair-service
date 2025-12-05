use nutype::nutype;
use serde::{Deserialize, Serialize};

use crate::domain::documents::DocumentId;

// structs

/// A uniquely identifiable document Attribute.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribute {
    pub id: AttributeId,
    pub body: AttributeBody
}

/// Attribute can be Field of Relation
/// in the future will be Component attribute type
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AttributeBody {
    Field {
        attribute_type: AttributeType,
        #[serde(default)]
        unique: bool,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        localized: bool,
        constraints: Option<AttributeConstraints>
    },
    Relation {
        relation_type: RelationType,
        target: DocumentId,
        ordering: bool,
        mapped_by: Option<AttributeId>,
        inversed_by: Option<AttributeId>
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

#[nutype(
    sanitize(trim, lowercase),
    validate(not_empty, len_char_max = 20, predicate = crate::domain::is_eligible_id),
    derive(
        Clone,
        Debug,
        Display,
        FromStr,
        AsRef,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Serialize
    )
)]
pub struct AttributeId(String);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeConstraints {
    pub pattern: Option<String>,
    pub minimal_length: Option<usize>,
    pub maximal_length: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RelationType {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}

// implementations

impl Attribute {
    pub fn new_field(
        id: AttributeId,
        attribute_type: AttributeType,
        unique: bool,
        required: bool,
        localized: bool,
        constraints: Option<AttributeConstraints>,
    ) -> Self {
        let body = AttributeBody::Field { attribute_type, unique, required, localized, constraints };
        Self {
            id,
            body
        }
    }
    
    pub fn new_relation(
        id: AttributeId,
        relation_type: RelationType,
        target: DocumentId,
        ordering: bool,
        mapped_by: Option<AttributeId>,
        inversed_by: Option<AttributeId>
    ) -> Self {
        let body = AttributeBody::Relation { relation_type, target, ordering, mapped_by, inversed_by };
        Self {
            id, 
            body
        }
    }
}

impl PartialEq for Attribute {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Attribute {}
