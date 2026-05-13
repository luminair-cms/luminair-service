use crate::domain::application::DocumentsService;
use luminair_common::DocumentTypesRegistry;

pub mod document;
pub mod repository;
pub mod application;

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
  
    type D: DocumentsService;
    
    fn document_types(&self) -> &'static dyn DocumentTypesRegistry;
    
    fn documents_service(&self) -> &Self::D;
}

