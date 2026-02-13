mod domain;
mod infratructure;

// Persisted documents field names

pub const ID_FIELD_NAME: &'static str = "id";
pub const DOCUMENT_ID_FIELD_NAME: &'static str = "document_id";
pub const RELATION_ID_FIELD_NAME: &'static str = "relation_id";

pub const CREATED_FIELD_NAME: &'static str = "created_at";
pub const UPDATED_FIELD_NAME: &'static str = "updated_at";
pub const PUBLISHED_FIELD_NAME: &'static str = "published_at";

pub const CREATED_BY_FIELD_NAME: &'static str = "created_by_id";
pub const UPDATED_BY_FIELD_NAME: &'static str = "updated_by_id";
pub const PUBLISHED_BY_FIELD_NAME: &'static str = "published_by_id";

pub const VERSION_FIELD_NAME: &'static str = "version";
pub const REVISION_FIELD_NAME: &'static str = "revision";

// expose domain module

pub use domain::*;
pub use infratructure::documents::load as load_documents;

// expose database module

pub use infratructure::database;
