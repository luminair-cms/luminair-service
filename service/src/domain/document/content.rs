use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::types::uuid;

#[derive(Debug, Clone)]
pub enum ContentValue {
    /// Simple scalar: text, number, boolean
    Scalar(DomainValue),

    /// Localized text: map of locale → text
    LocalizedText(HashMap<String, String>),
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

    /// JSON blob (still needed sometimes, but wrapped)
    Json(JsonBlob),

    /// Null value
    Null,
}

/// Newtype wrapper for email - enforces validation at domain level
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Email(String);

impl Email {
    pub fn new(value: String) -> Result<Self, EmailError> {
        // Validate email format
        if value.contains('@') && value.len() > 5 {
            Ok(Email(value))
        } else {
            Err(EmailError::InvalidFormat)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Newtype wrapper for URL
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Url(String);

impl Url {
    pub fn new(value: String) -> Result<Self, UrlError> {
        if value.starts_with("http://") || value.starts_with("https://") {
            Ok(Url(value))
        } else {
            Err(UrlError::InvalidScheme)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Wrapper for JSON blobs - typed but flexible when needed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonBlob {
    inner: serde_json::Value,
}

impl JsonBlob {
    pub fn new(value: serde_json::Value) -> Result<Self, JsonError> {
        // Validate JSON structure if needed
        Ok(JsonBlob { inner: value })
    }

    pub fn as_value(&self) -> &serde_json::Value {
        &self.inner
    }
}

#[derive(Debug)]
pub enum EmailError {
    InvalidFormat,
}

#[derive(Debug)]
pub enum UrlError {
    InvalidScheme,
}

#[derive(Debug)]
pub enum JsonError {
    InvalidStructure,
}
