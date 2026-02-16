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
    use crate::test_utils::{make_collection, SimpleRegistry};

    #[test]
    fn document_type_id_valid_and_normalize() {
        let dt = DocumentTypeId::try_new("My_Id-123").unwrap();
        // sanitized to lowercase and trimmed
        assert_eq!(dt.as_ref(), "my_id-123");
        assert_eq!(dt.normalized(), "my_id_123");
    }

    #[test]
    fn document_type_id_invalid_rejected() {
        assert!(DocumentTypeId::try_new("my id").is_err());
        assert!(DocumentTypeId::try_new("").is_err());
        assert!(DocumentTypeId::try_new("luminair_test").is_err());
    }

    #[test]
    fn attribute_id_normalize() {
        let a = AttributeId::try_new("Foo-Bar").unwrap();
        assert_eq!(a.as_ref(), "foo-bar");
        assert_eq!(a.normalized(), "foo_bar");
    }

    #[test]
    fn registry_iter_and_get() {
        let t1 = make_collection("alpha");
        let t2 = make_collection("beta");
        let reg = SimpleRegistry { types: vec![t1, t2] };
        let ids: Vec<_> = reg.iterate().map(|dt| dt.id.clone()).collect();
        assert_eq!(ids, vec![t1.id.clone(), t2.id.clone()]);
        assert_eq!(reg.get(&t1.id).unwrap().id, t1.id);
        assert!(reg.get(&DocumentTypeId::try_new("gamma").unwrap()).is_none());
    }
}

