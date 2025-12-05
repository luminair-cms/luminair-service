use std::sync::LazyLock;

use regex::Regex;

pub mod documents;
pub mod document_attributes;

// A regex for IDs/names that may contain only ASCII letters, digits, and underscore.
// Example: "My_Id_123" or "my-id" is valid; "my/id" or "my id" are not.
pub const ELIGIBLE_SYMBOLS_REGEX: &str = r"^[A-Za-z0-9_/-]+$";

static ELIGIBLE_SYMBOLS_REGEX_COMPILED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(ELIGIBLE_SYMBOLS_REGEX).expect("ELIGIBLE_SYMBOLS_REGEX must be a valid regex")
});

pub fn is_eligible_id(id: &str) -> bool {
    !id.starts_with("luminair_") && ELIGIBLE_SYMBOLS_REGEX_COMPILED.is_match(id)
}
