use std::fmt::Debug;
use std::sync::LazyLock;

use nutype::nutype;
use regex::Regex;

pub use crate::domain::entities::DocumentType;

pub mod entities;
pub mod persistence;

pub trait DocumentTypesRegistry: Send + Sync + Debug + 'static {
    
    /// iterate all documents metadata
    fn iterate(&self) -> Box<dyn Iterator<Item = &'static DocumentType> + '_>;
    
    /// find document metadata by its id
    fn get(&self, id: &DocumentTypeId) -> Option<&'static DocumentType>;

    /// Look up an API id and return the associated `DocumentType` if it exists.
    fn lookup(&self, api_id: &DocumentTypeApiId) -> Option<&'static DocumentType>;
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
pub struct DocumentTypeId(String);

impl DocumentTypeId {
    pub fn normalized(&self) -> String {
        self.as_ref().replace("-", "_")
    }
}

// validated api-id of a document type
// for Collection: plural form, for SingleType: singular form
#[nutype(
    sanitize(trim, lowercase),
    validate(not_empty, len_char_max = 20),
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
pub struct DocumentTypeApiId(String);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_type_id_normalizes_hyphens() {
        let id = DocumentTypeId::try_new("my-document").expect("valid id");
        assert_eq!(id.normalized(), "my_document");
    }

    #[test]
    fn attribute_id_normalizes_hyphens() {
        let id = AttributeId::try_new("my-attribute").expect("valid id");
        assert_eq!(id.normalized(), "my_attribute");
    }

    #[test]
    fn document_type_id_rejects_reserved_prefixes() {
        let result = DocumentTypeId::try_new("luminair_reserved");
        assert!(result.is_err(), "reserved prefix should be invalid");
    }

    #[test]
    fn document_type_id_rejects_invalid_symbols() {
        let result = DocumentTypeId::try_new("invalid symbol");
        assert!(result.is_err(), "spaces are not allowed in document type ids");
    }
}
