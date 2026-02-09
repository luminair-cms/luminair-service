use crate::domain::repository::DocumentInstanceRepository;
use luminair_common::DocumentTypesRegistry;

pub mod document;
pub mod repository;

/// This trait used only for testing purposes.
pub trait HelloService: Send + Sync + 'static {
    fn hello(&self) -> impl Future<Output = Result<String, anyhow::Error>> + Send;
}

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
    type H: HelloService;
    type R: DocumentInstanceRepository;

    fn hello_service(&self) -> &Self::H;
    fn document_types_registry(&self) -> &'static dyn DocumentTypesRegistry;
    fn documents_instance_repository(&self) -> &Self::R;
}
