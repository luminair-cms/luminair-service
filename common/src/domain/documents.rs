use std::{borrow::Borrow, hash::Hash, sync::LazyLock};

use nutype::nutype;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::domain::document_attributes::Attribute;

static VALID_LOCALIZATIONS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^(ru|ro|en)").unwrap());

pub trait Documents: Send + Sync + Clone + 'static {
    /// return documents metadata
    fn documents(&self) -> impl Iterator<Item = &Document>;
    /// find document by it's id
    fn get_document(&self, id: &DocumentId) -> Option<&Document>;
}

// structs

/// A uniquely identifiable Document.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: DocumentId,
    pub document_type: DocumentType,
    pub info: DocumentInfo,
    pub options: Option<DocumentOptions>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DocumentType {
    Collection,
    Singleton,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub title: DocumentTitle,
    pub description: DocumentDescription,
    pub singular_name: DocumentId,
    pub plural_name: DocumentId,
}

#[nutype(
    sanitize(trim, lowercase),
    validate(not_empty, len_char_max=20, predicate = crate::domain::is_eligible_id),
    derive(Clone, Debug, Display, FromStr, AsRef,
           PartialEq, Eq, PartialOrd, Ord, Hash,
           Serialize)
)]
pub struct DocumentId(String);

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

#[nutype(
    sanitize(trim),
    validate(not_empty),
    derive(
        Clone, Debug, Display, FromStr, AsRef, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize
    )
)]
pub struct DocumentDescription(String);

/// Options of document
#[derive(Clone, Debug, Serialize)]
pub struct DocumentOptions {
    pub draft_and_publish: bool,
    pub localizations: Vec<LocalizationId>,
}

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
        Serialize
    )
)]
pub struct LocalizationId(String);

// implementations

impl Document {
    pub fn new(
        id: DocumentId,
        document_type: DocumentType,
        info: DocumentInfo,
        options: Option<DocumentOptions>,
        attributes: Vec<Attribute>,
    ) -> Self {
        Self {
            id,
            document_type,
            info,
            options,
            attributes,
        }
    }

    pub fn has_localization(&self) -> bool {
        self.options.as_ref().map_or(false, |options|!options.localizations.is_empty())
    }
    
    pub fn has_draft_and_publish(&self) -> bool {
        self.options.as_ref().map_or(false, |options|options.draft_and_publish)
    }
    
    pub fn table_name(&self) -> String {
        self.id.as_ref().replace("-", "_")
    }

}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Document {}

impl PartialEq<DocumentId> for Document {
    fn eq(&self, other: &DocumentId) -> bool {
        self.id == *other
    }
}

impl Borrow<DocumentId> for Document {
    fn borrow(&self) -> &DocumentId {
        &self.id
    }
}

impl Hash for Document {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl DocumentInfo {
    pub fn new(
        title: DocumentTitle,
        description: DocumentDescription,
        singular_name: DocumentId,
        plural_name: DocumentId,
    ) -> Self {
        Self {
            title,
            description,
            singular_name,
            plural_name,
        }
    }
}

/*
impl From<&DocumentLocalization> for Vec<String> {
    fn from(value: &DocumentLocalization) -> Self {
        value.0.iter().map(LocalizationId::to_string).collect()
    }
}
*/
