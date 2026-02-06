use luminair_common::DocumentTypesRegistry;
use crate::domain::repository::DocumentInstanceRepository;

pub mod document;
pub mod repository;

/// This trait used only for testing purposes.
pub trait HelloService: Send + Sync + 'static {
    fn hello(&self) -> impl Future<Output = Result<String, anyhow::Error>> + Send;
}

//// The global application state shared between all request handlers.
pub trait AppState
{
    type H: HelloService;
    type S: DocumentTypesRegistry;
    type R: DocumentInstanceRepository;
    
    fn hello_service(&self) -> H;
    fn schema_registry(&self) -> S;
    fn repository(&self) -> R;
}
