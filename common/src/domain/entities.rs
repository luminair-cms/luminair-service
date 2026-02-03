use std::{
    borrow::Borrow, 
    collections::HashMap, 
    hash::Hash, 
    sync::LazyLock
};

use nutype::nutype;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::domain::{AttributeId, DocumentTypeId};

/// A DocumentType defines the structure/schema
/// Represents what KIND of document can exist
#[derive(Debug, Serialize)]
pub struct DocumentType {
    pub id: DocumentTypeId,
    pub kind: DocumentKind,
    pub info: DocumentTypeInfo,
    pub options: Option<DocumentTypeOptions>,
    pub fields: HashMap<AttributeId, DocumentField>,
    pub relations: HashMap<AttributeId, DocumentRelation>
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentKind {
    Collection,     // Many instances: Partners, Brands
    SingleType,     // One instance: Settings, SiteConfig
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTypeInfo {
    pub title: DocumentTitle,
    pub singular_name: DocumentTypeId,
    pub plural_name: DocumentTypeId,
    pub description: Option<String>,
}

#[nutype(
    sanitize(trim),
    validate(not_empty),
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
        Serialize,
        Deserialize
    )
)]
pub struct DocumentTitle(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTypeOptions {
    pub draft_and_publish: bool,
    pub localizations: Vec<LocalizationId>,
}

static VALID_LOCALIZATIONS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^(ru|ro|en)").unwrap());

#[nutype(
    sanitize(trim, lowercase),
    validate(not_empty, len_char_min = 2, len_char_max = 2, regex = VALID_LOCALIZATIONS_REGEX),
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
        Serialize,
        Deserialize
    )
)]
pub struct LocalizationId(String);


/// A uniquely identifiable document Field.
#[derive(Debug, Serialize)]
pub struct DocumentField {
    pub attribute_type: AttributeType,
    pub unique: bool,
    pub required: bool,
    pub localized: bool,
    pub constraints: Option<AttributeConstraints>,
}

/// A uniquely identifiable document Relation.
#[derive(Debug, Serialize)]
pub struct DocumentRelation {
    pub relation_type: RelationType,
    pub target: DocumentTypeId,
    pub ordering: bool
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


impl DocumentType {
    pub fn has_localization(&self) -> bool {
        self.options.as_ref().map_or(false, |options|!options.localizations.is_empty())
    }
    
    pub fn has_draft_and_publish(&self) -> bool {
        self.options.as_ref().map_or(false, |options|options.draft_and_publish)
    }
}

impl PartialEq for DocumentType {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for DocumentType {}

impl PartialEq<DocumentTypeId> for DocumentType {
    fn eq(&self, other: &DocumentTypeId) -> bool {
        self.id == *other
    }
}

impl Borrow<DocumentTypeId> for &DocumentType {
    fn borrow(&self) -> &DocumentTypeId {
        &self.id
    }
}

impl Hash for DocumentType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

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