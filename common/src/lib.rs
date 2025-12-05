use crate::{
    domain::documents::Documents, 
    infrastructure::documents::DocumentsAdapter
};

pub mod domain;
pub mod infrastructure;

pub fn load_documents(schema_config_path: &str) -> Result<impl Documents, anyhow::Error> {
    DocumentsAdapter::load(schema_config_path)
}