use std::sync::{Arc, OnceLock};
use crate::{
    domain::documents::Documents, 
    infrastructure::documents::DocumentsAdapter
};

pub mod domain;
pub mod infrastructure;

static DOCUMENTS: OnceLock<Arc<dyn Documents>> = OnceLock::new();

pub fn load_documents(schema_config_path: &str) -> Result<&'static dyn Documents, anyhow::Error> {
    let loaded = DocumentsAdapter::load(schema_config_path)?;
    // initiate relation references
    loaded.initiate()?;
    // store loaded documents in static variable
    DOCUMENTS.set(Arc::new(loaded)).expect("Failed to set documents");
    // get reference to Documents trait with static lifetime
    let documents: &'static dyn Documents = DOCUMENTS.get().unwrap().as_ref();
    
    Ok(documents)
}