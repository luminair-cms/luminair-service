use nutype::nutype;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    borrow::Borrow,
    hash::Hash,
    sync::LazyLock
};

use crate::domain::{AttributeId, DocumentTypeId};

/// A DocumentType defines the structure/schema
/// Represents what KIND of document can exist
#[derive(Debug, Serialize)]
pub struct DocumentType {
    pub id: DocumentTypeId,
    pub kind: DocumentKind,
    pub info: DocumentTypeInfo,
    pub options: Option<DocumentTypeOptions>,
    pub fields: HashSet<DocumentField>,
    pub relations: HashSet<DocumentRelation>
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
    pub id: AttributeId,
    pub field_type: FieldType,
    pub unique: bool,
    pub required: bool,
    pub constraints: HashSet<FieldConstraint>,
}

/// A uniquely identifiable document Relation.
#[derive(Debug, Serialize)]
pub struct DocumentRelation {
    pub id: AttributeId,
    pub relation_type: RelationType,
    pub target: DocumentTypeId
}

// TODO: support for more complex relations (e.g. with additional fields on the relation itself, like in a many-to-many with pivot table)
// TODO: support for self-relations (e.g. a "Category" that can have a parent category, which is also of type "Category")
// TODO: support for polymorphic relations (e.g. a "Comment" that can belong to either a "Post" or a "Product", etc.)
// TODO: support for recursive relations (e.g. a "Category" that can have subcategories, which are also of type "Category")
// TODO: support for more complex relation types (e.g. one-to-one, many-to-many, etc.) and relation options (e.g. cascade delete, etc.)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldType {
    Uid,  // unique identifier based on text
    Uuid, // unique identifier based on UUID
    Text,
    LocalizedText,
    Integer (
        #[serde(default)]
        IntegerSize
    ),
    Decimal {
        precision: usize,
        scale: u32
    },
    Date,
    DateTime,
    Boolean,
    Json,  // arbitrary JSON data
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum FieldConstraint {
    Pattern(String),     // test string with regular expression
    MinimalLength(usize), // test string with minimal length
    MaximalLength(usize), // test string with maximal length
    MinimalIntegerValue(i32),
    MaximalIntegerValue(i32)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum IntegerSize { Int16, Int32, Int64 }

impl Default for IntegerSize {
    fn default() -> Self {
        IntegerSize::Int32
    }
}

impl FieldType {
    pub fn is_integer(&self) -> bool {
        matches!(self, FieldType::Integer(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, FieldType::Integer(_) | FieldType::Decimal { .. })
    }

    pub fn is_text(&self) -> bool {
        matches!(self, FieldType::Text | FieldType::LocalizedText | FieldType::Uid)
    }
}

impl FieldConstraint {
    pub fn is_applicable_for(&self, field_type: FieldType) -> bool {
        match self {
            FieldConstraint::Pattern(_) => matches!(field_type, FieldType::Text | FieldType::Uid),
            FieldConstraint::MinimalLength(_) => field_type.is_text(),
            FieldConstraint::MaximalLength(_) => field_type.is_text(),
            FieldConstraint::MinimalIntegerValue(_) => field_type.is_integer(),
            FieldConstraint::MaximalIntegerValue(_) => field_type.is_integer(),
        }
    }
}

// TODO: support for more complex constraints (e.g. regex patterns for text, min/max for numbers, date ranges for dates, etc.)
// TODO: constraints that depend on other fields (e.g. "start_date" must be before "end_date", etc.)
// TODO: constraints that depend on the relation (e.g. "category" must be one of the categories defined in the "Category" document type, etc.)
// TODO: support for custom validation functions (e.g. a "validate_email" function that checks if a text field is a valid email address, etc.)
// TODO: support for localization-specific constraints (e.g. a "name" field that must be unique across all localizations, etc.)
// TODO: constraints that depends on the FieldType (e.g. a "price" field that must be a positive decimal, etc.)

// TODO: different constraints for different types (e.g. min/max for numbers, date ranges for dates, etc.)

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationType {
    // owning side
    HasOne,
    HasMany,
    // inverse side
    BelongsToOne,
    BelongsToMany,
}

// implementations

// Document

impl DocumentType {
    pub fn has_localization(&self) -> bool {
        self.options.as_ref().map_or(false, |options|!options.localizations.is_empty())
    }
    
    pub fn has_draft_and_publish(&self) -> bool {
        self.options.as_ref().map_or(false, |options|options.draft_and_publish)
    }

    pub fn ordered_fields(&self) -> Vec<&DocumentField> {
        // sord fields by unique flag, FieldType & name
        // order of types: integer, uuid, date, datetime, boolean, decimal, uid, text, localized text, json
        fn field_type_order(ft: &FieldType) -> u8 {
            match ft {
                FieldType::Integer(_) => 0,
                FieldType::Uuid => 1,
                FieldType::Date => 2,
                FieldType::DateTime => 3,
                FieldType::Boolean => 4,
                FieldType::Decimal { .. } => 5,
                FieldType::Uid => 6,
                FieldType::Text => 7,
                FieldType::LocalizedText => 8,
                FieldType::Json => 9,
            }
        }
        let mut fields: Vec<_> = self.fields.iter().collect();
        fields.sort_by_key(|f| (
            !f.unique, // unique fields first
            field_type_order(&f.field_type),
            &f.id
        ));
        fields
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

// Field

impl PartialEq for DocumentField {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for DocumentField {}

impl PartialEq<AttributeId> for DocumentField {
    fn eq(&self, other: &AttributeId) -> bool {
        self.id == *other
    }
}

impl Borrow<AttributeId> for DocumentField {
    fn borrow(&self) -> &AttributeId {
        &self.id
    }
}

impl Hash for DocumentField {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

// Relation

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

impl PartialEq for DocumentRelation {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for DocumentRelation {}

impl PartialEq<AttributeId> for DocumentRelation {
    fn eq(&self, other: &AttributeId) -> bool {
        self.id == *other
    }
}

impl Borrow<AttributeId> for DocumentRelation {
    fn borrow(&self) -> &AttributeId {
        &self.id
    }
}

impl Hash for DocumentRelation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}