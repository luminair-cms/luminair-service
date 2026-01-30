use std::fmt::Debug;
use std::sync::{Arc, LazyLock, OnceLock};
use nutype::nutype;
use regex::Regex;
use crate::documents::documents::Document;
use crate::documents::infrastructure::DocumentsAdapter;

pub mod attributes;
pub mod documents;
mod infrastructure;

static DOCUMENTS: OnceLock<Arc<dyn Documents>> = OnceLock::new();

pub fn load(schema_config_path: &str) -> Result<&'static dyn Documents, anyhow::Error> {
    let loaded = DocumentsAdapter::load(schema_config_path)?;
    // store loaded documents in static variable
   DOCUMENTS.set(Arc::new(loaded)).expect("Failed to set documents");
    // get reference to Documents trait with static lifetime
    let documents: &'static dyn Documents = DOCUMENTS.get().unwrap().as_ref();

    Ok(documents)
}

pub trait Documents: Send + Sync + Debug + 'static {
    /// iterate all documents metadata
    fn documents(&self) -> Box<dyn Iterator<Item = &'static Document> + '_>;
    /// find document metadata by its id
    fn get_document(&self, id: &DocumentId) -> Option<&'static Document>;
}

// A regex for IDs/names that may contain only ASCII letters, digits, and underscore.
// Example: "My_Id_123" or "my-id" is valid; "my/id" or "my id" are not.
const ELIGIBLE_SYMBOLS_REGEX: &str = r"^[A-Za-z0-9_/-]+$";

static ELIGIBLE_SYMBOLS_REGEX_COMPILED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(ELIGIBLE_SYMBOLS_REGEX).expect("ELIGIBLE_SYMBOLS_REGEX must be a valid regex")
});

fn is_eligible_id(id: &str) -> bool {
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
