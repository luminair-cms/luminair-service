pub mod implementation;
pub mod error;
pub mod commands;
pub mod service;

use luminair_common::DocumentTypesRegistry;
use crate::application::service::DocumentsService;

/// The global application state shared between all HTTP request handlers.
///
/// `AppState` is a composition-root concern: it wires the HTTP adapters to the
/// application service layer. It lives here rather than in the domain root because
/// it references [`DocumentsService`], which is an application-layer contract.
pub trait AppState: Clone + Send + Sync + 'static {
    type D: DocumentsService;

    fn document_types(&self) -> &'static dyn DocumentTypesRegistry;

    fn documents_service(&self) -> &Self::D;

    fn pagination_settings(&self) -> PaginationSettings;
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct PaginationSettings {
    pub default_page_size: u16,
    pub max_page_size: u16,
}

impl Default for PaginationSettings {
    fn default() -> Self {
        Self {
            default_page_size: 25,
            max_page_size: 100,
        }
    }
}

