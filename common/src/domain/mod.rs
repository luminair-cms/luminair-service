use std::fmt::Debug;
use std::sync::LazyLock;
use nutype::nutype;
use regex::Regex;
use crate::domain::documents::Document;
use crate::domain::persisted::PersistedDocument;

pub mod documents;
pub mod attributes;
pub mod persisted;

pub trait Documents: Send + Sync + Debug + 'static {
    /// iterate all documents metadata
    fn documents(&self) -> Box<dyn Iterator<Item = &Document> + '_>;
    /// find document metadata by its id
    fn get_document(&self, id: &DocumentId) -> Option<&Document>;
    /// iterate document persistence
    fn persisted_documents(&self) -> Box<dyn Iterator<Item = &PersistedDocument> + '_>;
    /// get document persistence by its id
    fn get_persisted_document(&self, id: &DocumentId) -> Option<&PersistedDocument>;
    /// get document persistence by its ref
    fn get_persisted_document_by_ref(&self, document_ref: DocumentRef) -> Option<&PersistedDocument>;
}

// A regex for IDs/names that may contain only ASCII letters, digits, and underscore.
// Example: "My_Id_123" or "my-id" is valid; "my/id" or "my id" are not.
pub const ELIGIBLE_SYMBOLS_REGEX: &str = r"^[A-Za-z0-9_/-]+$";

static ELIGIBLE_SYMBOLS_REGEX_COMPILED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(ELIGIBLE_SYMBOLS_REGEX).expect("ELIGIBLE_SYMBOLS_REGEX must be a valid regex")
});

pub fn is_eligible_id(id: &str) -> bool {
    !id.starts_with("luminair_") && ELIGIBLE_SYMBOLS_REGEX_COMPILED.is_match(id)
}

#[nutype(
    sanitize(trim, lowercase),
    validate(not_empty, len_char_max=20, predicate = is_eligible_id),
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
        Serialize)
)]
pub struct DocumentId(String);

impl DocumentId {
    pub fn normalized(&self) -> String {
        self.as_ref().replace("-", "_")
    }
}

#[nutype(
    sanitize(trim, lowercase),
    validate(not_empty, len_char_max = 20, predicate = is_eligible_id),
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
pub struct AttributeId(String);

impl AttributeId {
    pub fn normalized(&self) -> String {
        self.as_ref().replace("-", "_")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DocumentRef(usize);

impl DocumentRef {
    pub fn as_index(&self) -> usize {
        self.0
    }
}

impl From<usize> for DocumentRef {
    fn from(value: usize) -> Self {
        Self(value)
    }
}