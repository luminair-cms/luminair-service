use std::collections::HashMap;

use nutype::nutype;
use rust_decimal::Decimal;
use luminair_common::AttributeId;
use crate::domain::document::lifecycle::PublicationState;

/// The actual data payload of a document
#[derive(Debug, Clone)]
pub struct DocumentContent {
    /// All fields with their values
    pub fields: HashMap<AttributeId, ContentValue>,

    /// Publishing state (if draft_and_publish is enabled)
    pub publication_state: PublicationState,
}

impl DocumentContent {
    pub fn new(fields: HashMap<AttributeId, ContentValue>) -> Self {
        Self {
            fields,
            publication_state: PublicationState::Draft { revision: 0 },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContentValue {
    /// Simple scalar: text, number, boolean
    Scalar(DomainValue),

    /// Localized text: map of locale → text
    LocalizedText(HashMap<String, String>),
    
    /// Null value
    Null,
}

/// The actual domain value types your content can have
/// This is technology-agnostic, pure domain logic
#[derive(Debug, Clone, PartialEq)]
pub enum DomainValue {
    /// Text field
    Text(String),

    /// Integer field
    Integer(i64),

    /// Decimal/float field
    Decimal(Decimal),

    /// Boolean field
    Boolean(bool),

    /// Date field (YYYY-MM-DD)
    Date(chrono::NaiveDate),

    /// DateTime field
    DateTime(chrono::DateTime<chrono::Utc>),

    /// Email (validated)
    Email(Email),

    /// URL (validated)
    Url(Url),

    /// UUID
    Uuid(uuid::Uuid),

    /// JSON now very simple
    Json(HashMap<String, String>),
}

// Validate email format
fn is_valid_email(s: &str) -> bool {
    use email_address::EmailAddress;
    use std::str::FromStr;
    EmailAddress::from_str(s).is_ok()
}

#[nutype(
    // Sanitize by trimming whitespace and converting to lowercase
    sanitize(trim, lowercase),
    // Validate using our custom function
    validate(predicate = is_valid_email),
    // Derive useful traits like Debug, Clone, PartialEq, and optional Serde traits
    derive(Debug, Clone, PartialEq, Eq, AsRef, Hash, FromStr, Serialize, Deserialize)
)]
struct Email(String);

// A custom validation function that tries to parse the string into a valid Url
fn is_valid_url(s: &str) -> bool {
    use url::Url;
    use std::str::FromStr;
    Url::from_str(s).is_ok()
}

#[nutype(
    // You might want some sanitization like trim
    sanitize(trim),
    // Validate using our custom function
    validate(predicate = is_valid_url),
    // Derive useful traits
    derive(Debug, Clone, PartialEq, Eq, AsRef, Hash, FromStr, Serialize, Deserialize)
)]
struct Url(String);

