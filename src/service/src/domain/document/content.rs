use std::collections::HashMap;

use crate::domain::document::error::DocumentError;
use crate::domain::document::lifecycle::PublicationState;
use chrono::{DateTime, Utc};
use luminair_common::AttributeId;
use luminair_common::entities::{DocumentField, FieldConstraint, FieldType};
use nutype::nutype;
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

/// The actual data payload of a document.
#[derive(Debug, Clone)]
pub struct DocumentContent {
    /// All field values keyed by attribute ID.
    pub fields: HashMap<AttributeId, ContentValue>,
    /// Publication lifecycle state.
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

/// A single content value stored for a document field.
#[derive(Debug, Clone)]
pub enum ContentValue {
    /// A scalar typed value.
    Scalar(DomainValue),
    /// A locale-keyed text map (for `LocalizedText` fields).
    LocalizedText(HashMap<String, String>),
    /// Explicit absence of a value.
    Null,
}

/// Concrete domain value types — technology-agnostic, pure domain logic.
#[derive(Debug, Clone, PartialEq)]
pub enum DomainValue {
    Text(String),
    Integer(i64),
    Decimal(Decimal),
    Boolean(bool),
    Date(chrono::NaiveDate),
    DateTime(DateTime<Utc>),
    /// Validated email address (lower-cased, trimmed).
    Email(Email),
    /// Validated URL (trimmed).
    Url(Url),
    Uuid(uuid::Uuid),
    /// Flat JSON object stored as a string map.
    Json(HashMap<String, String>),
}

// ── JSON codec ──────────────────────────────────────────────────────────────
//
// All four field-level conversions (JSON→Domain, Domain→JSON, DB→Domain,
// Domain→DB) are driven by the same `FieldType` enum. Adding a new variant
// to `FieldType` in the `common` crate produces compile errors in every codec
// path, preventing silent gaps.
//
// JSON ↔ Domain lives here (domain is allowed to depend on serde_json).
// DB   ↔ Domain lives in `infrastructure/persistence/mapping/`.

impl ContentValue {
    /// Decode a JSON value into a [`ContentValue`] according to the field's declared type.
    ///
    /// ## Validation performed
    ///
    /// - If `value` is JSON `null` and `field.required` is `true`, returns
    ///   [`DocumentError::MissingRequiredField`].
    /// - All declared [`FieldConstraint`]s are applied after the type conversion.
    ///   Returns [`DocumentError::ConstraintViolation`] on the first failing constraint.
    ///
    /// ## Type mapping (JSON → domain)
    ///
    /// | `FieldType`     | Accepted JSON            | `DomainValue` variant |
    /// |-----------------|--------------------------|------------------------|
    /// | `Text`          | string                   | `Text`                 |
    /// | `Uid`           | string                   | `Text`                 |
    /// | `Uuid`          | UUID string              | `Uuid`                 |
    /// | `LocalizedText` | `{ "en": "…", … }`       | `LocalizedText`        |
    /// | `Integer`       | integer                  | `Integer`              |
    /// | `Decimal`       | number **or** string     | `Decimal`              |
    /// | `Boolean`       | boolean                  | `Boolean`              |
    /// | `Date`          | `"YYYY-MM-DD"`           | `Date`                 |
    /// | `DateTime`      | RFC 3339 string          | `DateTime`             |
    /// | `Json`          | object                   | `Json`                 |
    ///
    /// `Uid` maps to `DomainValue::Text`, not `Uuid`, because a Uid is a
    /// human-readable slug, not a UUID. See `FieldType::Uuid` for the UUID case.
    ///
    /// `Decimal` accepts both a JSON number and a quoted decimal string.
    /// The string form is preferred because it preserves full precision without
    /// rounding through `f64`.
    pub fn from_json(
        value: &serde_json::Value,
        field: &DocumentField,
    ) -> Result<Self, DocumentError> {
        if value.is_null() {
            return if field.required {
                Err(DocumentError::MissingRequiredField(field.id.to_string()))
            } else {
                Ok(ContentValue::Null)
            };
        }

        let content_value = Self::decode_type(value, field)?;

        // Apply all declared constraints after successful type conversion.
        for constraint in &field.constraints {
            if constraint.is_applicable_for(field.field_type) {
                Self::check_constraint(&content_value, constraint, field)?;
            }
        }

        Ok(content_value)
    }

    /// Decode `value` into the `DomainValue` variant dictated by `field.field_type`.
    fn decode_type(
        value: &serde_json::Value,
        field: &DocumentField,
    ) -> Result<ContentValue, DocumentError> {
        // Helper to build a typed error without repeating the field id.
        let err = |reason: &str| DocumentError::InvalidFieldValue {
            field: field.id.to_string(),
            reason: reason.to_owned(),
        };
        let errf = |reason: String| DocumentError::InvalidFieldValue {
            field: field.id.to_string(),
            reason,
        };

        match field.field_type {
            FieldType::Text => {
                let s = value
                    .as_str()
                    .ok_or_else(|| err("expected a string"))?
                    .to_owned();
                Ok(ContentValue::Scalar(DomainValue::Text(s)))
            }

            // Uid is a human-readable unique slug — stored and represented as text.
            // Do not map to DomainValue::Uuid; that is reserved for FieldType::Uuid.
            FieldType::Uid => {
                let s = value
                    .as_str()
                    .ok_or_else(|| err("expected a string"))?
                    .to_owned();
                Ok(ContentValue::Scalar(DomainValue::Text(s)))
            }

            FieldType::Uuid => {
                let s = value
                    .as_str()
                    .ok_or_else(|| err("expected a UUID string"))?;
                let uuid = uuid::Uuid::parse_str(s)
                    .map_err(|_| errf(format!("'{}' is not a valid UUID", s)))?;
                Ok(ContentValue::Scalar(DomainValue::Uuid(uuid)))
            }

            FieldType::LocalizedText => {
                let obj = value
                    .as_object()
                    .ok_or_else(|| err("expected an object with locale keys"))?;
                let mut map = HashMap::new();
                for (locale, v) in obj {
                    // TODO: validate locale is one of allowed locales for document type
                    let text = v
                        .as_str()
                        .ok_or_else(|| {
                            errf(format!("value for locale '{}' must be a string", locale))
                        })?
                        .to_owned();
                    map.insert(locale.clone(), text);
                }
                Ok(ContentValue::LocalizedText(map))
            }

            // Integer size variants (Int16/Int32/Int64) are all decoded as i64.
            // Range validation can be applied via FieldConstraint::MinimalIntegerValue
            // and FieldConstraint::MaximalIntegerValue if a narrower range is required.
            FieldType::Integer(_) => {
                let n = value.as_i64().ok_or_else(|| err("expected an integer"))?;
                Ok(ContentValue::Scalar(DomainValue::Integer(n)))
            }

            FieldType::Decimal { scale, .. } => {
                // Prefer the string form (full precision); fall back to JSON number (f64).
                let decimal: Decimal = if let Some(s) = value.as_str() {
                    s.parse::<Decimal>()
                        .map_err(|_| errf(format!("'{}' cannot be parsed as a decimal", s)))?
                } else {
                    let f = value
                        .as_f64()
                        .ok_or_else(|| err("expected a number or a quoted decimal string"))?;
                    Decimal::from_f64(f)
                        .ok_or_else(|| err("cannot represent value as a decimal"))?
                };
                let mut d = decimal;
                d.rescale(scale);
                Ok(ContentValue::Scalar(DomainValue::Decimal(d)))
            }

            FieldType::Boolean => {
                let b = value.as_bool().ok_or_else(|| err("expected a boolean"))?;
                Ok(ContentValue::Scalar(DomainValue::Boolean(b)))
            }

            FieldType::Date => {
                let s = value
                    .as_str()
                    .ok_or_else(|| err("expected a date string (YYYY-MM-DD)"))?;
                let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
                    errf(format!("'{}' is not a valid date (expected YYYY-MM-DD)", s))
                })?;
                Ok(ContentValue::Scalar(DomainValue::Date(date)))
            }

            FieldType::DateTime => {
                let s = value
                    .as_str()
                    .ok_or_else(|| err("expected an RFC 3339 datetime string"))?;
                let dt = chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| errf(format!("'{}' is not a valid RFC 3339 datetime", s)))?
                    .with_timezone(&chrono::Utc);
                Ok(ContentValue::Scalar(DomainValue::DateTime(dt)))
            }

            FieldType::Json => {
                let obj = value
                    .as_object()
                    .ok_or_else(|| err("expected a JSON object"))?;
                // JSON fields are stored as flat string maps.
                // Non-string values are serialised to their JSON representation.
                let map = obj
                    .iter()
                    .map(|(k, v)| {
                        let s = v
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string());
                        (k.clone(), s)
                    })
                    .collect();
                Ok(ContentValue::Scalar(DomainValue::Json(map)))
            }
        }
    }

    /// Validate a single [`FieldConstraint`] against an already-decoded value.
    fn check_constraint(
        value: &ContentValue,
        constraint: &FieldConstraint,
        field: &DocumentField,
    ) -> Result<(), DocumentError> {
        let violation = |reason: String| DocumentError::ConstraintViolation {
            field: field.id.to_string(),
            reason,
        };

        match (value, constraint) {
            (ContentValue::Scalar(DomainValue::Text(s)), FieldConstraint::MinimalLength(min)) => {
                if s.chars().count() < *min {
                    return Err(violation(format!(
                        "must be at least {} characters long",
                        min
                    )));
                }
            }
            (ContentValue::Scalar(DomainValue::Text(s)), FieldConstraint::MaximalLength(max)) => {
                if s.chars().count() > *max {
                    return Err(violation(format!("must not exceed {} characters", max)));
                }
            }
            (ContentValue::Scalar(DomainValue::Text(s)), FieldConstraint::Pattern(pattern)) => {
                let re = Regex::new(pattern).map_err(|_| {
                    violation(format!(
                        "constraint has an invalid regex pattern: '{}'",
                        pattern
                    ))
                })?;
                if !re.is_match(s) {
                    return Err(violation(format!("must match pattern '{}'", pattern)));
                }
            }
            (
                ContentValue::Scalar(DomainValue::Integer(n)),
                FieldConstraint::MinimalIntegerValue(min),
            ) => {
                if *n < i64::from(*min) {
                    return Err(violation(format!("must be at least {}", min)));
                }
            }
            (
                ContentValue::Scalar(DomainValue::Integer(n)),
                FieldConstraint::MaximalIntegerValue(max),
            ) => {
                if *n > i64::from(*max) {
                    return Err(violation(format!("must not exceed {}", max)));
                }
            }
            _ => {} // constraint not applicable to this value/constraint combination
        }
        Ok(())
    }
}

// ── Domain → JSON serialisation ──────────────────────────────────────────────

impl From<&ContentValue> for serde_json::Value {
    fn from(value: &ContentValue) -> Self {
        match value {
            ContentValue::Null => serde_json::Value::Null,
            ContentValue::LocalizedText(map) => serde_json::Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            ),
            ContentValue::Scalar(domain_value) => serde_json::Value::from(domain_value),
        }
    }
}

impl From<&DomainValue> for serde_json::Value {
    fn from(value: &DomainValue) -> Self {
        match value {
            DomainValue::Text(s) => serde_json::Value::String(s.clone()),
            DomainValue::Integer(n) => serde_json::Value::Number((*n).into()),
            DomainValue::Decimal(d) => {
                // Try to emit as a JSON number using the canonical decimal string.
                // Falls back to a JSON string if the number cannot be represented
                // (extremely rare for practical CMS content values).
                serde_json::from_str(&d.to_string())
                    .unwrap_or_else(|_| serde_json::Value::String(d.to_string()))
            }
            DomainValue::Boolean(b) => serde_json::Value::Bool(*b),
            DomainValue::Date(d) => serde_json::Value::String(d.to_string()),
            DomainValue::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
            DomainValue::Email(e) => serde_json::Value::String(e.as_ref().to_owned()),
            DomainValue::Url(u) => serde_json::Value::String(u.as_ref().to_owned()),
            DomainValue::Uuid(u) => serde_json::Value::String(u.to_string()),
            DomainValue::Json(map) => serde_json::Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            ),
        }
    }
}

// ── Validated value-object newtypes ─────────────────────────────────────────

fn is_valid_email(s: &str) -> bool {
    use email_address::EmailAddress;
    use std::str::FromStr;
    EmailAddress::from_str(s).is_ok()
}

#[nutype(
    sanitize(trim, lowercase),
    validate(predicate = is_valid_email),
    derive(Debug, Clone, PartialEq, Eq, AsRef, Hash, FromStr, Serialize, Deserialize)
)]
pub(crate) struct Email(String);

fn is_valid_url(s: &str) -> bool {
    use std::str::FromStr;
    use url::Url;
    Url::from_str(s).is_ok()
}

#[nutype(
    sanitize(trim),
    validate(predicate = is_valid_url),
    derive(Debug, Clone, PartialEq, Eq, AsRef, Hash, FromStr, Serialize, Deserialize)
)]
pub(crate) struct Url(String);
