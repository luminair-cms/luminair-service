mod domain;
mod infratructure;

// Persisted documents field names

pub const DOCUMENT_ID_FIELD_NAME: &str = "document_id";
pub const RELATION_ID_FIELD_NAME: &str = "relation_id";
pub const CREATED_FIELD_NAME: &str = "created_at";
pub const UPDATED_FIELD_NAME: &str = "updated_at";
pub const PUBLISHED_FIELD_NAME: &str = "published_at";

// expose domain module

pub use domain::*;
pub use infratructure::documents::load as load_documents;

// expose database module

pub use infratructure::database;
