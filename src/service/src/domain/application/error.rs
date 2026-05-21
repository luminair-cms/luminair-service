use crate::domain::document::error::DocumentError;
use crate::domain::repository::RepositoryError;

#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("Document type not found")]
    DocumentTypeNotFound,

    #[error("Document not found")]
    DocumentNotFound,

    #[error("Relation '{0}' not found")]
    RelationNotFound(String),

    #[error("Relation '{0}' is not an owning relation")]
    NotOwningRelation(String),

    #[error("Validation error: {0}")]
    Validation(#[from] DocumentError),

    #[error("Unique constraint violated: {0}")]
    Conflict(String),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<RepositoryError> for ServiceError {
    fn from(value: RepositoryError) -> Self {
        match value {
            RepositoryError::DocumentTypeNotFound => Self::DocumentTypeNotFound,
            RepositoryError::DocumentInstanceNotFound => Self::DocumentNotFound,
            RepositoryError::ValidationFailed(msg) => Self::Validation(DocumentError::InvalidFieldValue {
                field: "document".to_string(),
                reason: msg,
            }),
            RepositoryError::UniqueViolation(msg) => Self::Conflict(msg),
            RepositoryError::DatabaseError(msg) => Self::Internal(anyhow::anyhow!(msg)),
        }
    }
}