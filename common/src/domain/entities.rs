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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
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
    pub field_type: FieldType,
    pub unique: bool,
    pub required: bool,
    pub constraints: Option<AttributeConstraints>,
}

/// A uniquely identifiable document Relation.
#[derive(Debug, Serialize)]
pub struct DocumentRelation {
    pub relation_type: RelationType,
    pub target: DocumentTypeId
}

// TODO: support for more complex relations (e.g. with additional fields on the relation itself, like in a many-to-many with pivot table)
// TODO: support for self-relations (e.g. a "Category" that can have a parent category, which is also of type "Category")
// TODO: support for polymorphic relations (e.g. a "Comment" that can belong to either a "Post" or a "Product", etc.)
// TODO: support for recursive relations (e.g. a "Category" that can have subcategories, which are also of type "Category")
// TODO: support for more complex relation types (e.g. one-to-one, many-to-many, etc.) and relation options (e.g. cascade delete, etc.)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldType {
    Uid,  // unique identifier based on text
    Uuid, // unique identifier based on UUID
    Text {
        #[serde(skip)]
        localized: bool,
    },
    Integer,
    Decimal,
    Date,
    DateTime,
    Boolean,
    Json,  // arbitrary JSON data
}

// TODO: support for more complex constraints (e.g. regex patterns for text, min/max for numbers, date ranges for dates, etc.)
// TODO: constraints that depend on other fields (e.g. "start_date" must be before "end_date", etc.)
// TODO: constraints that depend on the relation (e.g. "category" must be one of the categories defined in the "Category" document type, etc.)
// TODO: support for custom validation functions (e.g. a "validate_email" function that checks if a text field is a valid email address, etc.)
// TODO: support for localization-specific constraints (e.g. a "name" field that must be unique across all localizations, etc.)
// TODO: constraints that depends on the FieldType (e.g. a "price" field that must be a positive decimal, etc.)

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct AttributeConstraints {
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub minimal_length: Option<usize>,
    #[serde(default)]
    pub maximal_length: Option<usize>,
}

// TODO: different constraints for different types (e.g. min/max for numbers, date ranges for dates, etc.)

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