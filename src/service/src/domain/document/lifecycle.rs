use chrono::{DateTime, Utc};
use nutype::nutype;

/// A validated user identifier.
///
/// Leading and trailing whitespace is trimmed before validation.
/// Empty strings are rejected — a meaningful user identity can never be blank.
///
/// # Construction
///
/// ```rust
/// # use service::domain::document::lifecycle::UserId;
/// let id = UserId::try_new("alice".to_string()).expect("non-empty");
/// ```
///
/// # Conversion
///
/// `UserId` implements `Into<String>`, so `.into()` gives the owned string.
/// `AsRef<str>` gives a borrowed view.
#[nutype(
    sanitize(trim),
    validate(not_empty),
    derive(
        Debug,
        Clone,
        Hash,
        Eq,
        PartialEq,
        AsRef,
        Into,
        Display,
        Serialize,
        Deserialize
    )
)]
pub struct UserId(String);

/// Publication lifecycle state of a document.
///
/// `revision` and `AuditTrail.version` are **independent counters** with
/// different purposes:
///
/// | Counter | Increments on | Answers |
/// |---------|---------------|---------|
/// | `AuditTrail.version` | every save (edit, publish, unpublish) | *how many times was this document modified?* |
/// | `revision` | publish only | *which publication of this document is this?* |
#[derive(Debug, Clone)]
pub enum PublicationState {
    /// The document is being edited and has not yet been (re-)published.
    ///
    /// `revision` holds the revision number of the **last publication this draft
    /// is based on**. For a document that has never been published, `revision = 0`.
    /// After unpublishing revision 3, the resulting draft carries `revision = 3`.
    Draft { revision: i32 },

    /// The document is live and publicly accessible.
    ///
    /// `revision` is a monotonically increasing publication counter starting at 1
    /// for the first publish. It is independent of `AuditTrail.version`.
    Published {
        revision: i32,
        published_at: DateTime<Utc>,
        published_by: Option<UserId>,
    },
}

/// System metadata: WHO did WHAT WHEN
/// This is infrastructure/audit concern, not domain logic
#[derive(Debug, Clone)]
pub struct AuditTrail {
    pub created_at: DateTime<Utc>,
    pub created_by: Option<UserId>,

    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<UserId>,

    pub version: i32,
}
