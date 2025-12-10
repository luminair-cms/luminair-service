use std::sync::LazyLock;
use nutype::nutype;
use regex::Regex;

pub mod documents;
pub mod attributes;
pub mod relations;

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
