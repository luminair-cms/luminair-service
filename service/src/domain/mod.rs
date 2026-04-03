use crate::domain::application::DocumentServices;
use luminair_common::DocumentTypesRegistry;

pub mod document;
pub mod repository;
pub mod application;

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
  
    type D: DocumentServices;
    
    fn document_types(&self) -> &'static dyn DocumentTypesRegistry;
    
    fn documents_services(&self) -> &Self::D;
}

