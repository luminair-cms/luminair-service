use std::collections::HashMap;

use nutype::nutype;
use sqlx::types::uuid;

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
    Decimal(f64),

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
pub struct Email(String);

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
pub struct Url(String);
