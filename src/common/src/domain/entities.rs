use nutype::nutype;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, collections::HashSet, hash::Hash, sync::LazyLock};

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
    pub relations: HashSet<DocumentRelation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    Collection, // Many instances: Partners, Brands
    SingleType, // One instance: Settings, SiteConfig
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
    pub target: DocumentTypeId,
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
    Integer(#[serde(default)] IntegerSize),
    Decimal { precision: usize, scale: u32 },
    Date,
    DateTime,
    Boolean,
    Json, // arbitrary JSON data
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum FieldConstraint {
    Pattern(String),      // test string with regular expression
    MinimalLength(usize), // test string with minimal length
    MaximalLength(usize), // test string with maximal length
    MinimalIntegerValue(i32),
    MaximalIntegerValue(i32),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum IntegerSize {
    Int16,
    #[default]
    Int32,
    Int64,
}

impl IntegerSize {
    pub fn to_sql_type(&self) -> &'static str {
        match self {
            IntegerSize::Int16 => "SMALLINT",
            IntegerSize::Int32 => "INT",
            IntegerSize::Int64 => "BIGINT",
        }
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
        matches!(
            self,
            FieldType::Text | FieldType::LocalizedText | FieldType::Uid
        )
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
    /// Creates a minimal `Collection` document type with no fields or relations.
    ///
    /// Useful in tests and examples where only the table identity matters.
    /// `id`, `singular`, and `plural` must each be a valid [`DocumentTypeId`] string.
    pub fn new_bare_collection(
        id: &str,
        singular: &str,
        plural: &str,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            id: DocumentTypeId::try_new(id)?,
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new(id)?,
                singular_name: DocumentTypeId::try_new(singular)?,
                plural_name: DocumentTypeId::try_new(plural)?,
                description: None,
            },
            options: None,
            fields: HashSet::new(),
            relations: HashSet::new(),
        })
    }

    pub fn has_localization(&self) -> bool {
        self.options
            .as_ref()
            .is_some_and(|options| !options.localizations.is_empty())
    }

    pub fn has_draft_and_publish(&self) -> bool {
        self.options
            .as_ref()
            .is_some_and(|options| options.draft_and_publish)
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
        fields.sort_by_key(|f| {
            (
                !f.unique, // unique fields first
                field_type_order(&f.field_type),
                &f.id,
            )
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fieldtype_predicates() {
        assert!(FieldType::Integer(IntegerSize::Int32).is_integer());
        assert!(FieldType::Integer(IntegerSize::Int16).is_number());
        assert!(
            FieldType::Decimal {
                precision: 10,
                scale: 2
            }
            .is_number()
        );
        assert!(FieldType::Text.is_text());
        assert!(FieldType::Uid.is_text());
    }

    #[test]
    fn fieldconstraint_applicability() {
        assert!(FieldConstraint::Pattern("x".into()).is_applicable_for(FieldType::Text));
        assert!(FieldConstraint::MinimalLength(1).is_applicable_for(FieldType::Uid));
        assert!(
            !FieldConstraint::MinimalLength(1)
                .is_applicable_for(FieldType::Integer(IntegerSize::Int32))
        );
        assert!(
            FieldConstraint::MinimalIntegerValue(0)
                .is_applicable_for(FieldType::Integer(IntegerSize::Int32))
        );
    }

    #[test]
    fn relation_type_flags() {
        assert!(RelationType::HasOne.is_owning());
        assert!(RelationType::HasMany.is_owning());
        assert!(RelationType::BelongsToOne.is_inverse());
        assert!(RelationType::BelongsToMany.is_inverse());
    }

    #[test]
    fn document_helpers_and_ordering_and_hashing() {
        // build a DocumentType with a few fields
        let title = DocumentTitle::try_new("My Type").unwrap();
        let singular = DocumentTypeId::try_new("mytype").unwrap();
        let plural = DocumentTypeId::try_new("mytypes").unwrap();
        let info = DocumentTypeInfo {
            title,
            singular_name: singular.clone(),
            plural_name: plural,
            description: None,
        };

        let mut fields = std::collections::HashSet::new();

        let id1 = AttributeId::try_new("a1").unwrap();
        let id2 = AttributeId::try_new("a2").unwrap();

        let f1 = DocumentField {
            id: id1.clone(),
            field_type: FieldType::Text,
            unique: true,
            required: false,
            constraints: Default::default(),
        };

        let f2 = DocumentField {
            id: id2.clone(),
            field_type: FieldType::Integer(IntegerSize::Int32),
            unique: false,
            required: false,
            constraints: Default::default(),
        };

        fields.insert(f1);
        fields.insert(f2);

        let doc = DocumentType {
            id: DocumentTypeId::try_new("mytype").unwrap(),
            kind: DocumentKind::Collection,
            info,
            options: None,
            fields,
            relations: Default::default(),
        };

        // has_localization false when options None
        assert!(!doc.has_localization());
        // has_draft_and_publish false when options None
        assert!(!doc.has_draft_and_publish());

        // ordered fields: unique first (id1), then integer (id2)
        let ordered = doc.ordered_fields();
        assert_eq!(ordered[0].id, id1);
        assert_eq!(ordered[1].id, id2);

        // hashing and equality: two docs with same id are equal
        let mut set = std::collections::HashSet::new();
        set.insert(doc);
        let dup = DocumentType {
            id: DocumentTypeId::try_new("mytype").unwrap(),
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new("Other").unwrap(),
                singular_name: DocumentTypeId::try_new("mytype").unwrap(),
                plural_name: DocumentTypeId::try_new("mytypes").unwrap(),
                description: None,
            },
            options: None,
            fields: Default::default(),
            relations: Default::default(),
        };
        // inserting duplicate by id should not increase set size
        assert!(!set.insert(dup));
    }
}
