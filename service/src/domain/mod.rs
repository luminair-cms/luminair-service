use luminair_common::domain::documents::Documents;

/// This trait used only for testing purposes.
pub trait HelloService: Send + Sync + 'static {
    fn hello(&self) -> impl Future<Output = Result<String, anyhow::Error>> + Send;
}

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
    type H: HelloService;
    fn hello_service(&self) -> &Self::H;
    fn documents(&self) -> &'static dyn Documents;
}