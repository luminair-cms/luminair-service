#[derive(Debug)]
pub enum DocumentError {
    MissingRequiredField(String),
    InvalidFieldValue(String),
    AlreadyPublished,
    AlreadyDraft,
    ValidationFailed(String),
}