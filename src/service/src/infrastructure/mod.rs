use crate::application::AppState;
use crate::application::implementation::DocumentsServiceImpl;
use crate::infrastructure::persistence::repository::PostgresDocumentsRepository;
use luminair_common::DocumentTypesRegistry;

pub mod http;
pub mod persistence;
pub mod settings;

#[derive(Clone)]
pub struct AppStateImpl {
    types: &'static dyn DocumentTypesRegistry,
    documents_service: DocumentsServiceImpl<PostgresDocumentsRepository>,
    pagination_settings: crate::application::PaginationSettings,
}

impl AppStateImpl {
    pub fn new(
        types: &'static dyn DocumentTypesRegistry,
        documents_repository: PostgresDocumentsRepository,
        pagination_settings: crate::application::PaginationSettings,
    ) -> Self {
        Self {
            types,
            documents_service: DocumentsServiceImpl::new(documents_repository),
            pagination_settings,
        }
    }
}

impl AppState for AppStateImpl {
    type D = DocumentsServiceImpl<PostgresDocumentsRepository>;

    fn document_types(&self) -> &'static dyn DocumentTypesRegistry {
        self.types
    }

    fn documents_service(&self) -> &Self::D {
        &self.documents_service
    }

    fn pagination_settings(&self) -> crate::application::PaginationSettings {
        self.pagination_settings
    }
}
