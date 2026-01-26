use std::collections::HashMap;
use std::{borrow::Borrow, hash::Hash, sync::LazyLock};
use std::fmt::Debug;
use nutype::nutype;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::domain::attributes::{DocumentField, DocumentRelation};
use crate::domain::{AttributeId, DocumentId};

static VALID_LOCALIZATIONS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^(ru|ro|en)").unwrap());

// structs

/// A uniquely identifiable Document.
#[derive(Debug)]
pub struct Document {
    pub id: DocumentId,
    pub document_type: DocumentType,
    pub info: DocumentInfo,
    pub options: Option<DocumentOptions>,
    pub persistence: DocumentPersistence,
    pub fields: HashMap<AttributeId, DocumentField>,
    pub relations: HashMap<AttributeId, DocumentRelation>
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

#[derive(Debug)]
pub struct DocumentPersistence {
    pub main_table_name: String,
    pub relation_column_name: String,
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

#[nutype(
    sanitize(trim),
    validate(not_empty),
    derive(
        Clone, Debug, Display, FromStr, AsRef, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize
    )
)]
pub struct DocumentDescription(String);

/// Options of document
#[derive(Clone, Debug)]
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
        Hash
    )
)]
pub struct LocalizationId(String);

// implementations

impl Document {
    pub fn has_localization(&self) -> bool {
        self.options.as_ref().map_or(false, |options|!options.localizations.is_empty())
    }
    
    pub fn has_draft_and_publish(&self) -> bool {
        self.options.as_ref().map_or(false, |options|options.draft_and_publish)
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

impl Borrow<DocumentId> for &Document {
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
