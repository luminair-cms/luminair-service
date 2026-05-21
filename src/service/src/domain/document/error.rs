/// Domain errors for document operations.
///
/// These errors represent violations of business rules and invariants within the
/// document lifecycle. They are converted to [`crate::application::error::ServiceError`]
/// at the application layer boundary, and never exposed directly to HTTP callers.
#[derive(thiserror::Error, Debug)]
pub enum DocumentError {
    /// A field marked as `required` in the schema was absent from the payload.
    #[error("Missing required field: '{0}'")]
    MissingRequiredField(String),

    /// The supplied value for a field does not match the declared `FieldType`.
    #[error("Invalid value for field '{field}': {reason}")]
    InvalidFieldValue { field: String, reason: String },

    /// A `FieldConstraint` (pattern, min/max length, min/max value) was violated.
    #[error("Constraint violated for field '{field}': {reason}")]
    ConstraintViolation { field: String, reason: String },

    /// Attempted to publish a document that is already in the `Published` state.
    /// Use `unpublish` first if re-publishing is intended.
    #[error("Document is already published")]
    AlreadyPublished,

    /// Attempted to unpublish a document that is already in the `Draft` state.
    #[error("Document is already a draft")]
    AlreadyDraft,
}
